use bson::{Bson, Document, doc};
use tracing::instrument;

pub const PARTITION_SIZE_IN_BYTES: i64 = 100 * 1024 * 1024; // 100 MB

/// Returns true when `min` and `max` can be compared using match-language range operators.
fn bounds_are_comparable(min: &Bson, max: &Bson) -> bool {
    fn is_wildcard(bound: &Bson) -> bool {
        matches!(bound, Bson::MinKey | Bson::MaxKey)
    }

    fn is_numeric(bound: &Bson) -> bool {
        matches!(
            bound,
            Bson::Int32(_) | Bson::Int64(_) | Bson::Double(_) | Bson::Decimal128(_)
        )
    }

    is_wildcard(min)
        || is_wildcard(max)
        || (is_numeric(min) && is_numeric(max))
        || min.element_type() == max.element_type()
}

/// Returns the string `$type` reports for `bound`, for use in `$expr` type predicates.
fn type_name(bound: &Bson) -> &'static str {
    match bound {
        Bson::Double(_) => "double",
        Bson::String(_) => "string",
        Bson::Array(_) => "array",
        Bson::Document(_) => "object",
        Bson::Boolean(_) => "bool",
        Bson::Null => "null",
        Bson::RegularExpression(_) => "regex",
        Bson::JavaScriptCode(_) => "javascript",
        Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        Bson::Int32(_) => "int",
        Bson::Int64(_) => "long",
        Bson::Timestamp(_) => "timestamp",
        Bson::Binary(_) => "binData",
        Bson::ObjectId(_) => "objectId",
        Bson::DateTime(_) => "date",
        Bson::Symbol(_) => "symbol",
        Bson::Decimal128(_) => "decimal",
        Bson::Undefined => "undefined",
        Bson::MaxKey => "maxKey",
        Bson::MinKey => "minKey",
        Bson::DbPointer(_) => "dbPointer",
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Partition {
    pub min: Bson,
    pub max: Bson,
    pub is_max_bound_inclusive: bool,
}

impl Partition {
    // generate_partition_match generates the $match stage for sampling based on the partition, an
    // optional schema, and a list of _id values to ignore. If the Schema is None, the $match will
    // only be based on the Partition bounds and the ignored_ids list.
    // If the min and max bounds are not comparable in match language, the $expr language will be used.
    //
    // Both branches share the same logic: a document is in the partition if its partition key type
    // matches the type of one of the bounds.
    #[instrument(level = "trace", skip_all)]
    pub fn generate_match(
        &self,
        doc: Option<Document>,
        ignored_ids: &[Bson],
        partition_key: &str,
    ) -> Document {
        let lt_op = if self.is_max_bound_inclusive {
            "$lte"
        } else {
            "$lt"
        };

        // If the min and max bounds are not comparable in match language, fall back to the
        // $expr language
        if !bounds_are_comparable(&self.min, &self.max) {
            let key_path = format!("${partition_key}");
            // Each bound constrains only documents of that bound's own BSON type. Note that partition key
            // values whose type matches neither bound are excluded.
            // So if the bound types are (ObjectId, String) but some ids are Int32, those ids will be excluded.
            let mut expr_body = doc! {
                "$expr": {
                    "$and": [
                        // $literal keeps a $-prefixed ignored value from being read as a
                        // field path.
                        doc! {"$not": {"$in": [&key_path, {"$literal": ignored_ids.to_vec()}]}},
                        doc! {
                            "$or": [
                                doc! {"$and": [
                                    doc! {"$eq": [{"$type": &key_path}, type_name(&self.min)]},
                                    doc! {"$gte": [&key_path, self.min.clone()]},
                                ]},
                                doc! {"$and": [
                                    doc! {"$eq": [{"$type": &key_path}, type_name(&self.max)]},
                                    doc! {lt_op: [&key_path, self.max.clone()]},
                                ]},
                            ]
                        },
                    ]
                }
            };

            // $jsonSchema has no $expr equivalent, so the schema exclusion stays in match
            // language as a sibling of $expr, which is valid within a single $match.
            if let Some(schema) = doc {
                expr_body.insert("$nor", vec![schema]);
            }

            doc! {
                "$match": expr_body
            }
        } else {
            let mut match_body = doc! {
                partition_key: {
                    "$nin": ignored_ids,
                    "$gte": self.min.clone(),
                    lt_op: self.max.clone(),
                }
            };
            if let Some(schema) = doc {
                match_body.insert("$nor", vec![schema]);
            }
            doc! {
                "$match": match_body
            }
        }
    }
}
