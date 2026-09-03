use crate::{
    air::{self, AggregationFunction, ProjectItem},
    codegen::{MqlCodeGenerator, MqlTranslation, Result},
};
use bson::{bson, doc, Bson};

impl MqlCodeGenerator {
    pub fn codegen_stage(&self, stage: air::Stage) -> Result<MqlTranslation> {
        match stage {
            air::Stage::AddFields(a) => self.codegen_add_fields(a),
            air::Stage::Project(p) => self.codegen_project(p),
            air::Stage::Group(g) => self.codegen_group(g),
            air::Stage::Limit(l) => self.codegen_limit(l),
            air::Stage::Sort(s) => self.codegen_sort(s),
            air::Stage::Collection(c) => self.codegen_collection(c),
            air::Stage::Join(j) => self.codegen_join(j),
            air::Stage::Unwind(u) => self.codegen_unwind(u),
            air::Stage::Lookup(l) => self.codegen_lookup(l),
            air::Stage::ReplaceWith(r) => self.codegen_replace_with(r),
            air::Stage::Match(m) => self.codegen_match(m),
            air::Stage::UnionWith(u) => self.codegen_union_with(u),
            air::Stage::Skip(s) => self.codegen_skip(s),
            air::Stage::Documents(d) => self.codegen_documents(d),
            air::Stage::EquiJoin(j) => self.codegen_equijoin(j),
            air::Stage::EquiLookup(l) => self.codegen_equilookup(l),
            air::Stage::SetWindowFields(s) => self.codegen_set_window_fields(s),
            air::Stage::Sentinel => unreachable!(),
        }
    }

    fn codegen_set_window_fields(
        &self,
        air_set_window_fields: air::SetWindowFields,
    ) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_set_window_fields.source)?;
        let mut pipeline = source_translation.pipeline;

        let mut body = doc! {};
        if let Some(partition_by) = air_set_window_fields.partition_by {
            body.insert("partitionBy", self.codegen_expression(partition_by)?);
        }
        if let Some(sort_by) = air_set_window_fields.sort_by {
            body.insert("sortBy", Self::codegen_sort_specs(sort_by));
        }

        let output = air_set_window_fields
            .output_fields
            .into_iter()
            .map(
                |air::SetWindowFieldsOutputField {
                     name,
                     window_function,
                 }| {
                    Ok((
                        name,
                        Bson::Document(self.codegen_window_function(window_function)?),
                    ))
                },
            )
            .collect::<Result<bson::Document>>()?;
        body.insert("output", output);

        pipeline.push(doc! {"$setWindowFields": body});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    /// Renders a single window operator, e.g. `{"$sum": "$quantity", "window": {...}}`.
    fn codegen_window_function(&self, func: air::WindowFunction) -> Result<bson::Document> {
        match func {
            air::WindowFunction::Aggregation(agg) => {
                // $group wraps accumulator arguments in an array via
                // codegen_mql_semantic_operator; window operators take the argument
                // directly, so this deliberately does not reuse that path.
                let mut doc = doc! {
                    Self::agg_func_to_mql_op(agg.function):
                        self.codegen_expression(*agg.expression)?,
                };
                if let Some(window) = agg.window {
                    doc.insert("window", Self::codegen_window_bounds(window));
                }
                Ok(doc)
            }
            // $count takes an empty document, like the rank operators.
            air::WindowFunction::Count => Ok(doc! { "$count": {} }),
            // Rank operators take an empty document and derive their window from the sort.
            air::WindowFunction::Rank(rank) => Ok(doc! { Self::rank_func_to_mql_op(rank): {} }),
            air::WindowFunction::Shift(shift) => {
                let mut args = doc! {
                    "output": self.codegen_expression(*shift.output)?,
                    "by": shift.by,
                };
                // MongoDB defaults an omitted `default` to null.
                if let Some(default) = shift.default {
                    args.insert("default", self.codegen_expression(*default)?);
                }
                Ok(doc! { "$shift": args })
            }
        }
    }

    fn codegen_window_bounds(bounds: air::WindowBounds) -> bson::Document {
        match bounds {
            air::WindowBounds::Documents(range) => {
                doc! {"documents": Self::codegen_window_range(range)}
            }
            air::WindowBounds::Range(range) => doc! {"range": Self::codegen_window_range(range)},
            air::WindowBounds::TimeRange(air::TimeRange { bounds, unit }) => doc! {
                "range": Self::codegen_window_range(bounds),
                // A window `unit` is a bare string, unlike the wrapped literal that
                // date_part_to_mql_unit produces for $dateAdd and friends.
                "unit": unit.to_str(),
            },
        }
    }

    fn codegen_window_range(range: air::WindowRange) -> Bson {
        Bson::Array(vec![
            Self::codegen_window_boundary(range.lower),
            Self::codegen_window_boundary(range.upper),
        ])
    }

    fn codegen_window_boundary(boundary: air::WindowBoundary) -> Bson {
        match boundary {
            air::WindowBoundary::Unbounded => Bson::String("unbounded".to_string()),
            air::WindowBoundary::Current => Bson::String("current".to_string()),
            air::WindowBoundary::Position(n) => Bson::Int64(n),
        }
    }

    fn codegen_union_with(&self, air_union_with: air::UnionWith) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_union_with.source)?;
        let mut pipeline = source_translation.pipeline;

        let pipeline_translation = self.codegen_stage(*air_union_with.pipeline)?;

        let mut union_body = doc! {};
        if let Some(collection) = pipeline_translation.collection {
            union_body.extend(doc! {"coll": collection});
        }
        union_body.extend(doc! {"pipeline": pipeline_translation.pipeline});

        pipeline.push(doc! {"$unionWith": union_body});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    /// Renders sort specifications as the `{<key>: 1 | -1}` document shared by the
    /// `$sort` stage and the `sortBy` field of `$setWindowFields`.
    fn codegen_sort_specs(specs: Vec<air::SortSpecification>) -> bson::Document {
        use air::SortSpecification::*;

        specs
            .into_iter()
            .map(|spec| match spec {
                Asc(key) => (key, Bson::Int32(1)),
                Desc(key) => (key, Bson::Int32(-1)),
            })
            .collect()
    }

    fn codegen_sort(&self, air_sort: air::Sort) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_sort.source)?;
        let mut pipeline = source_translation.pipeline;
        let sort_specs = Self::codegen_sort_specs(air_sort.specs);

        pipeline.push(doc! {"$sort": sort_specs});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_match(&self, air_match: air::Match) -> Result<MqlTranslation> {
        match air_match {
            air::Match::ExprLanguage(e) => self.codegen_match_expr_language(e),
            air::Match::MatchLanguage(m) => self.codegen_match_match_language(m),
        }
    }

    fn codegen_match_expr_language(
        &self,
        air_match_expr_language: air::ExprLanguage,
    ) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_match_expr_language.source)?;
        let mut pipeline = source_translation.pipeline;
        let expr = bson!({ "$expr": self.codegen_expression(*air_match_expr_language.expr)?});

        pipeline.push(doc! {"$match": expr});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_match_match_language(
        &self,
        air_match_match_language: air::MatchLanguage,
    ) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_match_match_language.source)?;
        let mut pipeline = source_translation.pipeline;
        let condition = self.codegen_match_query(*air_match_match_language.expr)?;

        pipeline.push(doc! {"$match": condition});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_replace_with(&self, air_replace_with: air::ReplaceWith) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_replace_with.source)?;
        let mut pipeline = source_translation.pipeline;
        let expr = self.codegen_expression(*air_replace_with.new_root)?;

        pipeline.push(doc! {"$replaceWith": expr});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_documents(&self, air_docs: air::Documents) -> Result<MqlTranslation> {
        let docs = air_docs
            .array
            .into_iter()
            .map(|e| self.codegen_expression(e))
            .collect::<Result<Vec<Bson>>>()?;
        Ok(MqlTranslation {
            database: None,
            collection: None,
            pipeline: vec![doc! {"$documents": Bson::Array(docs)}],
        })
    }

    fn codegen_collection(&self, air_coll: air::Collection) -> Result<MqlTranslation> {
        Ok(MqlTranslation {
            database: Some(air_coll.db),
            collection: Some(air_coll.collection),
            pipeline: vec![],
        })
    }

    fn codegen_lookup(&self, air_lookup: air::Lookup) -> Result<MqlTranslation> {
        let lookup_pipeline_translation = self.codegen_stage(*air_lookup.pipeline)?;
        let source_translation = self.codegen_stage(*air_lookup.source)?;
        let mut pipeline = source_translation.pipeline;
        let mut lookup_doc = match (
            lookup_pipeline_translation.database,
            lookup_pipeline_translation.collection,
        ) {
            (Some(from_db), Some(from_coll)) => {
                if Some(&from_db) == source_translation.database.as_ref() {
                    doc! {"from": from_coll}
                } else {
                    doc! {"from": {"db": from_db, "coll": from_coll}}
                }
            }
            _ => doc! {},
        };
        if let Some(let_vars) = air_lookup.let_vars {
            lookup_doc.extend(doc! { "let":
                let_vars
                    .into_iter()
                    .map(|v| Ok((v.name, self.codegen_expression(*v.expr)?)))
                    .collect::<Result<bson::Document>>()?
            });
        }
        lookup_doc.extend(doc! {
            "pipeline": lookup_pipeline_translation.pipeline,
            "as": air_lookup.as_var,
        });
        pipeline.push(doc! {"$lookup": lookup_doc});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_project(&self, air_project: air::Project) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_project.source)?;
        let mut pipeline = source_translation.pipeline;
        let project_doc = air_project
            .specifications
            .into_iter()
            .map(|(k, v)| Ok((k, self.codegen_project_item(v)?)))
            .collect::<Result<bson::Document>>()?;
        pipeline.push(doc! {"$project": project_doc});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_project_item(&self, air_project_item: air::ProjectItem) -> Result<Bson> {
        match air_project_item {
            ProjectItem::Exclusion => Ok(Bson::Int32(0)),
            ProjectItem::Inclusion => Ok(Bson::Int32(1)),
            ProjectItem::Assignment(e) => self.codegen_expression(e),
        }
    }

    fn codegen_add_fields(&self, air_add_fields: air::AddFields) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_add_fields.source)?;
        let mut pipeline = source_translation.pipeline;
        let add_fields_doc = air_add_fields
            .specifications
            .into_iter()
            .map(|(k, v)| {
                let it = self.codegen_expression(v)?;
                Ok((k, it))
            })
            .collect::<Result<bson::Document>>()?;
        pipeline.push(doc! {"$addFields": add_fields_doc});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_group(&self, air_group: air::Group) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_group.source)?;
        let mut pipeline = source_translation.pipeline;
        let id_doc = air_group
            .keys
            .into_iter()
            .map(|air::NameExprPair { name: k, expr: v }| Ok((k, self.codegen_expression(v)?)))
            .collect::<Result<bson::Document>>()?;
        let mut group_doc = doc! {"_id": id_doc};
        let aggs = air_group
            .aggregations
            .into_iter()
            .map(
                |air::AccumulatorExpr {
                     alias,
                     function,
                     distinct,
                     arg,
                    arg_is_possibly_doc: _,
                 }| {
                    Ok(if function == AggregationFunction::AddToArray {
                        if distinct {
                            (alias, bson!({ "$addToSet": self.codegen_expression(*arg)? }))
                        }
                        else{
                            (alias, bson!({ "$push": self.codegen_expression(*arg)? }))
                        }
                    }
                    else if distinct || function == AggregationFunction::Count {
                        (alias, bson!({ Self::agg_func_to_sql_op(function): {"var": self.codegen_expression(*arg)?, "distinct": distinct }}))
                    } else {
                        (alias, bson!({ Self::agg_func_to_mql_op(function): self.codegen_expression(*arg)? }))
                    })
                },
            )
            .collect::<Result<bson::Document>>()?;
        group_doc.extend(aggs);
        pipeline.push(doc! {"$group": group_doc});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_unwind(&self, air_unwind: air::Unwind) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_unwind.source)?;
        let mut pipeline = source_translation.pipeline;
        let mut include_array_index = air_unwind.index;
        // If the `path` field_ref has a parent, we need to crawl up the parent chain
        // and find the root parent and append the argument to `includeArrayIndex` to that.
        // If the path does not have a parent, then we're already at the root level of the document
        // and can just use the argument as is.
        if include_array_index.is_some() {
            if let Some(field_ref) = match air_unwind.path {
                air::Expression::FieldRef(ref f) => Some(f),
                _ => None,
            } {
                if field_ref.parent.is_some() {
                    include_array_index = Some(format!(
                        "{}.{}",
                        field_ref.root_parent(),
                        include_array_index.unwrap()
                    ));
                }
            }
        }
        let path = self.codegen_expression(air_unwind.path)?;
        let preserve_null_and_empty_arrays = air_unwind.outer;

        let unwind_body = match (
            include_array_index.is_some(),
            preserve_null_and_empty_arrays,
        ) {
            (true, true) => {
                doc! {"path": path, "includeArrayIndex": include_array_index.unwrap(), "preserveNullAndEmptyArrays": preserve_null_and_empty_arrays}
            }
            (true, false) => doc! {"path": path, "includeArrayIndex": include_array_index.unwrap()},
            (false, true) => {
                doc! {"path": path, "preserveNullAndEmptyArrays": preserve_null_and_empty_arrays}
            }
            (false, false) => doc! {"path": path},
        };

        pipeline.push(doc! {"$unwind": unwind_body});

        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_skip(&self, air_skip: air::Skip) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_skip.source)?;
        let mut pipeline = source_translation.pipeline;
        pipeline.push(doc! {"$skip": Bson::Int64(air_skip.skip)});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    fn codegen_limit(&self, air_limit: air::Limit) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_limit.source)?;
        let mut pipeline = source_translation.pipeline;
        pipeline.push(doc! {"$limit": Bson::Int64(air_limit.limit)});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }

    #[allow(clippy::unnecessary_unwrap)]
    fn codegen_join(&self, air_join: air::Join) -> Result<MqlTranslation> {
        let mut left_translation = self.codegen_stage(*air_join.left)?;
        let right_translation = self.codegen_stage(*air_join.right)?;
        let join_type = match air_join.join_type {
            air::JoinType::Inner => "inner",
            air::JoinType::Left => "left",
        };
        let mut join_doc = doc! {};
        if let Some(right_db) = right_translation.database.clone() {
            if left_translation.database != right_translation.database {
                join_doc.insert("database", right_db);
            }
        }

        if right_translation.collection.is_some() {
            join_doc.insert("collection", right_translation.collection);
        }

        join_doc.insert("joinType", join_type);

        if air_join.let_vars.is_some() {
            join_doc.insert(
                "let",
                air_join
                    .let_vars
                    .unwrap()
                    .iter()
                    .map(|v| Ok((v.name.clone(), self.codegen_expression(*v.expr.clone())?)))
                    .collect::<Result<bson::Document>>()?,
            );
        }

        join_doc.insert("pipeline", right_translation.pipeline);
        if air_join.condition.is_some() {
            let cond = self.codegen_expression(air_join.condition.unwrap())?;
            join_doc.insert("condition", cond);
        }

        left_translation.pipeline.push(doc! {"$join": join_doc});
        Ok(left_translation)
    }

    fn codegen_equijoin(&self, air_join: air::EquiJoin) -> Result<MqlTranslation> {
        let mut source_translation = self.codegen_stage(*air_join.source)?;
        let join_type = match air_join.join_type {
            air::JoinType::Inner => "inner",
            air::JoinType::Left => "left",
        };
        let mut join_doc = doc! {};
        join_doc.insert("database", air_join.from.db);
        join_doc.insert("collection", air_join.from.collection);
        join_doc.insert(
            "localField",
            self.codegen_field_ref_path_only(air_join.local_field),
        );
        join_doc.insert(
            "foreignField",
            self.codegen_field_ref_path_only(air_join.foreign_field),
        );
        join_doc.insert("joinType", join_type);
        join_doc.insert("as", air_join.as_name);

        source_translation
            .pipeline
            .push(doc! {"$equiJoin": join_doc});
        Ok(source_translation)
    }

    fn codegen_equilookup(&self, air_lookup: air::EquiLookup) -> Result<MqlTranslation> {
        let source_translation = self.codegen_stage(*air_lookup.source)?;
        let from = air_lookup.from;
        let mut lookup_doc = if Some(&from.db) == source_translation.database.as_ref() {
            doc! {"from": from.collection}
        } else {
            doc! {"from": {"db": from.db, "coll": from.collection}}
        };
        lookup_doc.extend(doc! {
            "localField": self.codegen_field_ref_path_only(air_lookup.local_field),
            "foreignField": self.codegen_field_ref_path_only(air_lookup.foreign_field),
            "as": air_lookup.as_var,
        });
        let mut pipeline = source_translation.pipeline;
        pipeline.push(doc! {"$lookup": lookup_doc});
        Ok(MqlTranslation {
            database: source_translation.database,
            collection: source_translation.collection,
            pipeline,
        })
    }
}
