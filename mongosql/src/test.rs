#[allow(clippy::redundant_pattern_matching)]
mod test_get_namespaces {
    macro_rules! test_get_namespaces {
        ($func_name:ident, $(expected = $expected:expr,)? $(expected_pat = $expected_pat:pat,)? db = $current_db:expr, query = $sql:expr,) => {
            #[test]
            fn $func_name() {
                #[allow(unused_imports)]
                use crate::{get_namespaces, set};
                let current_db = $current_db;
                let sql = $sql;
                let actual = get_namespaces(current_db, sql);
                $(assert!(matches!(actual, $expected_pat));)?
                $(assert_eq!($expected, actual);)?
            }
        };
    }

    test_get_namespaces!(
        no_collections,
        expected = Ok(set![]),
        db = "mydb",
        query = "select * from [] as arr",
    );

    test_get_namespaces!(
        implicit,
        expected = Ok(set![agg_ast::definitions::Namespace {
            database: "mydb".into(),
            collection: "foo".into()
        }]),
        db = "mydb",
        query = "select * from foo",
    );

    test_get_namespaces!(
        explicit,
        expected = Ok(set![agg_ast::definitions::Namespace {
            database: "bar".into(),
            collection: "baz".into()
        }]),
        db = "mydb",
        query = "select * from bar.baz",
    );

    test_get_namespaces!(
        duplicates,
        expected = Ok(set![agg_ast::definitions::Namespace {
            database: "mydb".into(),
            collection: "foo".into()
        }]),
        db = "mydb",
        query = "select * from foo a join foo b",
    );

    test_get_namespaces!(
        semantically_invalid,
        expected = Ok(set![
            agg_ast::definitions::Namespace {
                database: "mydb".into(),
                collection: "foo".into()
            },
            agg_ast::definitions::Namespace {
                database: "mydb".into(),
                collection: "bar".into()
            }
        ]),
        db = "mydb",
        query = "select a from foo join bar",
    );

    test_get_namespaces!(
        syntactically_invalid,
        expected_pat = Err(_),
        db = "mydb",
        query = "not a valid query",
    );
}

mod test_mql_schema_env_to_json_schema {
    use crate::{
        json_schema::{self, BsonType, BsonTypeName},
        map,
        mapping_registry::*,
        mql_schema_env_to_json_schema,
        options::{ExcludeNamespacesOption::*, SqlOptions},
        result::Error::Translator,
        schema::*,
        set,
        translator::Error::{DocumentSchemaTypeNotFound, ReferenceNotFound},
        Schema, SchemaCheckingMode,
    };

    macro_rules! test_mql_schema_env_to_json_schema {
        ($func_name:ident,
         schema_env = $schema_env:expr,
         mapping_registry = $mapping_registry:expr,
         sql_options = $sql_options:expr,
         $(expected = $expected:expr,)? $(expected_pat = $expected_pat:pat,)?) => {
            #[test]
            fn $func_name() {
                let result = mql_schema_env_to_json_schema(
                    $schema_env,
                    &$mapping_registry,
                    $sql_options,
                );

                $(assert!(matches!(result, $expected_pat));)?
                $(assert_eq!($expected, result.unwrap());)?
            }
        };
    }

    test_mql_schema_env_to_json_schema!(
        reference_not_found_in_mapping_registry,
        schema_env = map! {
            ("foo", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "a".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "a".into() },
                additional_properties: false,
                ..Default::default()
                }),
            ("bar", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "b".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "b".into() },
                additional_properties: false,
                ..Default::default()
                }),
        },
        mapping_registry = {
            let mut mr = MqlMappingRegistry::default();
            mr.insert(
                ("foobar", 0u16),
                MqlMappingRegistryValue::new("foo".to_string(), MqlReferenceType::FieldRef),
            );
            mr.insert(
                ("bar", 0u16),
                MqlMappingRegistryValue::new("bar".to_string(), MqlReferenceType::FieldRef),
            );
            mr
        },
        sql_options = SqlOptions::new(ExcludeNamespaces, SchemaCheckingMode::default()),
        expected_pat = Err(Translator(ReferenceNotFound(_))),
    );

    test_mql_schema_env_to_json_schema!(
        document_schema_type_not_found_in_schema_env,
        schema_env = map! {
            ("foo", 0u16).into() => Schema::Atomic(Atomic::Integer),
            ("bar", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "b".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "b".into() },
                additional_properties: false,
                ..Default::default()
                }),
        },
        mapping_registry = {
            let mut mr = MqlMappingRegistry::default();
            mr.insert(
                ("foo", 0u16),
                MqlMappingRegistryValue::new("foo".to_string(), MqlReferenceType::FieldRef),
            );
            mr.insert(
                ("bar", 0u16),
                MqlMappingRegistryValue::new("bar".to_string(), MqlReferenceType::FieldRef),
            );
            mr
        },
        sql_options = SqlOptions::new(ExcludeNamespaces, SchemaCheckingMode::default()),
        expected_pat = Err(Translator(DocumentSchemaTypeNotFound(Schema::Atomic(
            Atomic::Integer
        )))),
    );

    test_mql_schema_env_to_json_schema!(
        include_namespaces_in_result_set_schema,
        schema_env = map! {
            ("foo", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "a".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "a".into() },
                additional_properties: false,
                ..Default::default()
                }),
            ("bar", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "b".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "b".into() },
                additional_properties: false,
                ..Default::default()
                }),
        },
        mapping_registry = {
            let mut mr = MqlMappingRegistry::default();
            mr.insert(
                ("foo", 0u16),
                MqlMappingRegistryValue::new("foo".to_string(), MqlReferenceType::FieldRef),
            );
            mr.insert(
                ("bar", 0u16),
                MqlMappingRegistryValue::new("bar".to_string(), MqlReferenceType::FieldRef),
            );
            mr
        },
        sql_options = SqlOptions::new(IncludeNamespaces, SchemaCheckingMode::default()),
        expected = json_schema::Schema {
            bson_type: Some(BsonType::Single(BsonTypeName::Object)),
            properties: Some(map! {
                "bar".to_string() => json_schema::Schema {
                    bson_type: Some(BsonType::Single(BsonTypeName::Object)),
                    properties: Some(map!{
                        "b".to_string() =>json_schema::Schema {
                            bson_type: Some(BsonType::Single(BsonTypeName::String)),
                            ..Default::default()
                        }
                    }),
                    required: Some(vec!["b".to_string()]),
                    additional_properties: Some(false),
                    ..Default::default()
                },
                "foo".to_string() => json_schema::Schema {
                    bson_type: Some(BsonType::Single(BsonTypeName::Object)),
                    properties: Some(map!{
                        "a".to_string() => json_schema::Schema {
                            bson_type: Some(BsonType::Single(BsonTypeName::String)),
                            ..Default::default()
                        }
                    }),
                    required: Some(vec!["a".to_string()]),
                    additional_properties: Some(false),
                    ..Default::default()
                }
            }),
            required: Some(vec!["bar".to_string(), "foo".to_string()]),
            additional_properties: Some(false),
            ..Default::default()
        },
    );

    test_mql_schema_env_to_json_schema!(
        exclude_namespaces_in_result_set_schema,
        schema_env = map! {
            ("foo", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "a".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "a".into() },
                additional_properties: false,
                ..Default::default()
                }),
            ("bar", 0u16).into() => Schema::Document( Document{
                keys: map! {
                    "b".into() => Schema::Atomic(Atomic::String)
                },
                required: set! { "b".into() },
                additional_properties: false,
                ..Default::default()
                }),
        },
        mapping_registry = {
            let mut mr = MqlMappingRegistry::default();
            mr.insert(
                ("foo", 0u16),
                MqlMappingRegistryValue::new("foo".to_string(), MqlReferenceType::FieldRef),
            );
            mr.insert(
                ("bar", 0u16),
                MqlMappingRegistryValue::new("bar".to_string(), MqlReferenceType::FieldRef),
            );
            mr
        },
        sql_options = SqlOptions::new(ExcludeNamespaces, SchemaCheckingMode::default()),
        expected = json_schema::Schema {
            bson_type: Some(BsonType::Single(BsonTypeName::Object)),
            properties: Some(map! {
                "a".to_string() => json_schema::Schema {
                    bson_type: Some(BsonType::Single(BsonTypeName::String)),
                    ..Default::default()
                },
                "b".to_string() => json_schema::Schema {
                    bson_type: Some(BsonType::Single(BsonTypeName::String)),
                    ..Default::default()
                }
            }),
            required: Some(vec!["a".to_string(), "b".to_string()]),
            additional_properties: Some(false),
            ..Default::default()
        },
    );
}

mod test_get_select_order {
    use crate::{ast, get_select_order};

    macro_rules! test_get_select_order {
        ($func_name:ident,
         expected = $expected:expr,
         input = $input:expr
        ) => {
            #[test]
            fn $func_name() {
                let option = get_select_order($input);
                assert_eq!($expected, option);
            }
        };
    }

    test_get_select_order!(
        select_body_standard_is_some,
        expected = Some(ast::SelectBody::Standard(vec![])),
        input = &ast::Query::Select(Box::new(ast::SelectQuery {
            select_clause: ast::SelectClause {
                set_quantifier: ast::SetQuantifier::All,
                body: ast::SelectBody::Standard(vec![]),
            },
            from_clause: None,
            where_clause: None,
            group_by_clause: None,
            order_by_clause: None,
            having_clause: None,
            limit: None,
            offset: None
        }))
    );

    test_get_select_order!(
        select_distinct_is_some,
        expected = Some(ast::SelectBody::Standard(vec![])),
        input = &ast::Query::Select(Box::new(ast::SelectQuery {
            select_clause: ast::SelectClause {
                set_quantifier: ast::SetQuantifier::Distinct,
                body: ast::SelectBody::Standard(vec![]),
            },
            from_clause: None,
            where_clause: None,
            group_by_clause: None,
            order_by_clause: None,
            having_clause: None,
            limit: None,
            offset: None
        }))
    );
}

mod select_list_order {
    use crate::{
        catalog::Catalog,
        map,
        schema::{Atomic, Document, Schema},
    };
    use agg_ast::definitions::Namespace;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref CATALOG: Catalog = Catalog::new(map! {
            Namespace {database: "test".to_string(), collection: "foo".to_string()} => Schema::Document(Document {
                keys: map! {
                    "a".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                    "b".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                    "c".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                },
                required: map!{},
                additional_properties: false,
                ..Default::default()
                }),
            Namespace {database: "test".to_string(), collection: "bar".to_string()} => Schema::Document(Document {
                keys: map! {
                    "a".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                    "b".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                    "c".to_string() => Schema::Array(Box::new(Schema::Atomic(Atomic::Integer))),
                },
                required: map!{},
                additional_properties: false,
                ..Default::default()
                }),
        });
    }

    macro_rules! test_parse_select_list_order {
        ($func_name:ident, sql = $sql:expr, exclude_namespaces = $exclude_namespaces:expr, expected = $expected:expr) => {
            #[test]
            fn $func_name() {
                #[allow(unused_imports)]
                use crate::{
                    translate_sql, ExcludeNamespacesOption, SchemaCheckingMode, SqlOptions,
                };
                let translation = translate_sql(
                    "test",
                    $sql,
                    &*CATALOG,
                    SqlOptions {
                        schema_checking_mode: SchemaCheckingMode::default(),
                        exclude_namespaces: $exclude_namespaces,
                        allow_order_by_missing_columns: false,
                    },
                );
                assert!(translation.is_ok());
                assert_eq!(translation.unwrap().select_order, $expected)
            }
        };
    }

    test_parse_select_list_order!(
        star_sorted,
        sql = "select * from foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["foo".to_string(), "a".to_string()],
            vec!["foo".to_string(), "b".to_string()],
            vec!["foo".to_string(), "c".to_string()]
        ]
    );

    test_parse_select_list_order!(
        star_multiple_collections_sorted,
        sql = "select * from foo, bar",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["bar".to_string(), "a".to_string()],
            vec!["bar".to_string(), "b".to_string()],
            vec!["bar".to_string(), "c".to_string()],
            vec!["foo".to_string(), "a".to_string()],
            vec!["foo".to_string(), "b".to_string()],
            vec!["foo".to_string(), "c".to_string()],
        ]
    );

    test_parse_select_list_order!(
        substar_simple,
        sql = "select foo.* from foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["foo".to_string(), "a".to_string()],
            vec!["foo".to_string(), "b".to_string()],
            vec!["foo".to_string(), "c".to_string()]
        ]
    );

    test_parse_select_list_order!(
        one_collection_non_alphabetical,
        sql = "select c, a, b from foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "c".to_string()],
            vec!["".to_string(), "a".to_string()],
            vec!["".to_string(), "b".to_string()]
        ]
    );

    test_parse_select_list_order!(
        fields_from_two_collections,
        sql = " select foo.a, bar.b from bar, foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "a".to_string()],
            vec!["".to_string(), "b".to_string()]
        ]
    );

    test_parse_select_list_order!(
        substar_between_fields,
        sql = " select foo.a, bar.*, foo.b from bar, foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "a".to_string()],
            vec!["bar".to_string(), "a".to_string()],
            vec!["bar".to_string(), "b".to_string()],
            vec!["bar".to_string(), "c".to_string()],
            vec!["".to_string(), "b".to_string()]
        ]
    );

    test_parse_select_list_order!(
        multiple_substars,
        sql = " select foo.*, bar.* from bar, foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["foo".to_string(), "a".to_string()],
            vec!["foo".to_string(), "b".to_string()],
            vec!["foo".to_string(), "c".to_string()],
            vec!["bar".to_string(), "a".to_string()],
            vec!["bar".to_string(), "b".to_string()],
            vec!["bar".to_string(), "c".to_string()],
        ]
    );

    test_parse_select_list_order!(
        fields_aliased_datasources,
        sql = " select f.a, b.b, f.c from bar as b, foo as f",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "a".to_string()],
            vec!["".to_string(), "b".to_string()],
            vec!["".to_string(), "c".to_string()],
        ]
    );

    test_parse_select_list_order!(
        substar_with_aliased_datasources,
        sql = " select f.a, b.*, f.b from bar as b, foo as f",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "a".to_string()],
            vec!["b".to_string(), "a".to_string()],
            vec!["b".to_string(), "b".to_string()],
            vec!["b".to_string(), "c".to_string()],
            vec!["".to_string(), "b".to_string()],
        ]
    );

    test_parse_select_list_order!(
        aliased_fields,
        sql = " select foo.a as f_a, bar.a as b_a from foo, bar",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "f_a".to_string()],
            vec!["".to_string(), "b_a".to_string()],
        ]
    );

    test_parse_select_list_order!(
        aggregations_and_fields,
        sql = "select foo.a, sum(bar.a), foo.b,  count(bar.b) from foo, bar group by foo.a, foo.b",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "a".to_string()],
            vec!["".to_string(), "_2".to_string()],
            vec!["".to_string(), "b".to_string()],
            vec!["".to_string(), "_4".to_string()],
        ]
    );

    test_parse_select_list_order!(
        values_simple,
        sql = "select values {'b': foo.b, 'a': 1} from foo",
        exclude_namespaces = ExcludeNamespacesOption::IncludeNamespaces,
        expected = vec![
            vec!["".to_string(), "b".to_string()],
            vec!["".to_string(), "a".to_string()],
        ]
    );
    test_parse_select_list_order!(
        star_no_reordering_exclude_namespaces,
        sql = "select * from foo",
        exclude_namespaces = ExcludeNamespacesOption::ExcludeNamespaces,
        expected = vec![
            vec!["a".to_string()],
            vec!["b".to_string()],
            vec!["c".to_string()]
        ]
    );

    test_parse_select_list_order!(
        one_collection_non_alphabetical_exclude_namespaces,
        sql = "select c, a, b from foo",
        exclude_namespaces = ExcludeNamespacesOption::ExcludeNamespaces,
        expected = vec![
            vec!["c".to_string()],
            vec!["a".to_string()],
            vec!["b".to_string()]
        ]
    );

    test_parse_select_list_order!(
        fields_from_two_collections_exclude_namespaces,
        sql = " select foo.a, bar.b from bar, foo",
        exclude_namespaces = ExcludeNamespacesOption::ExcludeNamespaces,
        expected = vec![vec!["a".to_string()], vec!["b".to_string()]]
    );
}

/// End-to-end SQL to MQL tests for window functions.
///
/// These assert the `$setWindowFields` stages rather than a MIR tree, because the parts most
/// likely to regress are how window functions are bucketed into stages and how the generated
/// `_wfN` outputs relate to the user's own aliases.
mod window_functions {
    use crate::{
        catalog::Catalog,
        map,
        schema::{Atomic, Document, Schema},
        translate_sql, SqlOptions,
    };
    use agg_ast::definitions::Namespace;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref CATALOG: Catalog = Catalog::new(map! {
            Namespace {database: "test".to_string(), collection: "foo".to_string()} =>
                Schema::Document(Document {
                    keys: map! {
                        "a".to_string() => Schema::Atomic(Atomic::Integer),
                        "b".to_string() => Schema::Atomic(Atomic::Integer),
                        "c".to_string() => Schema::Atomic(Atomic::String),
                    },
                    required: map! {},
                    additional_properties: false,
                    ..Default::default()
                }),
            Namespace {database: "test".to_string(), collection: "stock_price".to_string()} =>
                Schema::Document(Document {
                    keys: map! {
                        "order_date".to_string() => Schema::Atomic(Atomic::Date),
                        "price".to_string() => Schema::Atomic(Atomic::Double),
                    },
                    required: map! {},
                    additional_properties: false,
                    ..Default::default()
                }),
        });
    }

    /// The `$setWindowFields` stages of the translated pipeline, as extended JSON strings.
    fn window_stages(sql: &str) -> Vec<String> {
        let t = translate_sql("test", sql, &*CATALOG, SqlOptions::default())
            .unwrap_or_else(|e| panic!("{sql} failed to translate: {e}"));
        t.pipeline
            .as_array()
            .expect("pipeline should be an array")
            .iter()
            .filter_map(|s| s.as_document().and_then(|d| d.get("$setWindowFields")))
            .map(|s| s.to_string().replace(' ', ""))
            .collect()
    }

    /// The final `$project` before the result is unwrapped, which is where the user's own
    /// aliases appear.
    fn final_projection(sql: &str) -> String {
        let t = translate_sql("test", sql, &*CATALOG, SqlOptions::default())
            .unwrap_or_else(|e| panic!("{sql} failed to translate: {e}"));
        t.pipeline
            .as_array()
            .expect("pipeline should be an array")
            .iter()
            .filter_map(|s| s.as_document().and_then(|d| d.get("$project")))
            .last()
            .expect("expected a $project stage")
            .to_string()
            .replace(' ', "")
    }

    fn error(sql: &str) -> String {
        translate_sql("test", sql, &*CATALOG, SqlOptions::default())
            .err()
            .unwrap_or_else(|| panic!("{sql} unexpectedly translated"))
            .to_string()
    }

    macro_rules! window_stages_test {
        ($name:ident, sql = $sql:expr, expected = $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!($expected.to_vec(), window_stages($sql));
            }
        };
    }

    macro_rules! error_test {
        ($name:ident, sql = $sql:expr, expected = $expected:expr) => {
            #[test]
            fn $name() {
                assert_eq!($expected.to_string(), error($sql));
            }
        };
    }

    window_stages_test!(
        no_partition_or_sort,
        sql = "SELECT SUM(a) OVER () AS total FROM foo",
        expected = [r#"{"output":{"__bot._wf1":{"$sum":"$foo.a"}}}"#]
    );

    window_stages_test!(
        partition_and_sort,
        sql = "SELECT SUM(a) OVER (PARTITION BY b ORDER BY a) AS running FROM foo",
        expected = [
            r#"{"partitionBy":"$foo.b","sortBy":{"foo.a":1},"output":{"__bot._wf1":{"$sum":"$foo.a"}}}"#
        ]
    );

    // Several partition keys become a document, the way $group builds a compound _id.
    window_stages_test!(
        multiple_partition_keys_become_a_document,
        sql = "SELECT SUM(a) OVER (PARTITION BY b, c) AS s FROM foo",
        expected = [
            r#"{"partitionBy":{"_partition0":"$foo.b","_partition1":"$foo.c"},"output":{"__bot._wf1":{"$sum":"$foo.a"}}}"#
        ]
    );

    // Sharing a specification means sharing a stage, even though the frames differ.
    window_stages_test!(
        same_specification_shares_one_stage,
        sql =
            "SELECT SUM(a) OVER (PARTITION BY b) AS s, AVG(a) OVER (PARTITION BY b) AS av FROM foo",
        expected = [
            r#"{"partitionBy":"$foo.b","output":{"__bot._wf1":{"$sum":"$foo.a"},"__bot._wf2":{"$avg":"$foo.a"}}}"#
        ]
    );

    // Differing specifications cannot share a stage, because $setWindowFields carries only
    // one partitionBy/sortBy, so they become a chain.
    window_stages_test!(
        different_specifications_chain_stages,
        sql = "SELECT SUM(a) OVER (PARTITION BY b) AS s, RANK() OVER (ORDER BY a) AS r FROM foo",
        expected = [
            r#"{"partitionBy":"$foo.b","output":{"__bot._wf1":{"$sum":"$foo.a"}}}"#,
            r#"{"sortBy":{"foo.a":1},"output":{"__bot._wf2":{"$rank":{}}}}"#
        ]
    );

    // The frame is per output field, so two functions with different frames still share a stage.
    window_stages_test!(
        differing_frames_share_a_stage,
        sql = "SELECT SUM(a) OVER (ORDER BY b ROWS BETWEEN 2 PRECEDING AND CURRENT ROW) AS w, \
               SUM(a) OVER (ORDER BY b) AS t FROM foo",
        expected = [
            r#"{"sortBy":{"foo.b":1},"output":{"__bot._wf1":{"$sum":"$foo.a","window":{"documents":[-2,"current"]}},"__bot._wf2":{"$sum":"$foo.a"}}}"#
        ]
    );

    window_stages_test!(
        count_star_uses_the_count_operator,
        sql = "SELECT COUNT(*) OVER () AS n FROM foo",
        expected = [r#"{"output":{"__bot._wf1":{"$count":{}}}}"#]
    );

    // LAG looks backwards, so the offset is negated; LEAD does not.
    window_stages_test!(
        lag_negates_the_offset,
        sql = "SELECT LAG(a, 1) OVER (ORDER BY b) AS prev FROM foo",
        expected = [
            r#"{"sortBy":{"foo.b":1},"output":{"__bot._wf1":{"$shift":{"output":"$foo.a","by":-1}}}}"#
        ]
    );

    window_stages_test!(
        lead_with_default,
        sql = "SELECT LEAD(a, 2, 0) OVER (ORDER BY b) AS nxt FROM foo",
        expected = [
            r#"{"sortBy":{"foo.b":1},"output":{"__bot._wf1":{"$shift":{"output":"$foo.a","by":2,"default":{"$literal":0}}}}}"#
        ]
    );

    window_stages_test!(
        omitted_shift_offset_defaults_to_one,
        sql = "SELECT LAG(a) OVER (ORDER BY b) AS prev FROM foo",
        expected = [
            r#"{"sortBy":{"foo.b":1},"output":{"__bot._wf1":{"$shift":{"output":"$foo.a","by":-1}}}}"#
        ]
    );

    window_stages_test!(
        row_number_maps_to_document_number,
        sql = "SELECT ROW_NUMBER() OVER (PARTITION BY c ORDER BY a DESC) AS rn FROM foo",
        expected = [
            r#"{"partitionBy":"$foo.c","sortBy":{"foo.a":-1},"output":{"__bot._wf1":{"$documentNumber":{}}}}"#
        ]
    );

    window_stages_test!(
        range_frame,
        sql = "SELECT SUM(a) OVER (ORDER BY b RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS s FROM foo",
        expected =
            [r#"{"sortBy":{"foo.b":1},"output":{"__bot._wf1":{"$sum":"$foo.a","window":{"range":["unbounded","current"]}}}}"#]
    );

    // The stage is additive, so ordinary columns selected alongside a window function survive.
    #[test]
    fn non_window_columns_survive() {
        assert_eq!(
            r#"{"__bot":{"a":"$foo.a","s":"$__bot._wf1"},"_id":0}"#,
            final_projection("SELECT a, SUM(a) OVER (PARTITION BY b) AS s FROM foo")
        );
    }

    mod aliases {
        use super::*;

        // The user's alias appears in the projection; `_wfN` stays internal to the stage.
        #[test]
        fn user_alias_is_projected_from_the_internal_name() {
            assert_eq!(
                r#"{"__bot":{"total":"$__bot._wf1"},"_id":0}"#,
                final_projection("SELECT SUM(a) OVER () AS total FROM foo")
            );
        }

        #[test]
        fn several_aliases_over_one_stage() {
            let sql =
                "SELECT SUM(a) OVER (PARTITION BY b) AS s, AVG(a) OVER (PARTITION BY b) AS av FROM foo";
            assert_eq!(
                r#"{"__bot":{"s":"$__bot._wf1","av":"$__bot._wf2"},"_id":0}"#,
                final_projection(sql)
            );
        }

        // Two textually identical window calls collapse to one output field but keep both
        // user aliases, so de-duplication is invisible to the caller.
        #[test]
        fn duplicate_expressions_share_one_output_field() {
            let sql = "SELECT SUM(a) OVER () AS x, SUM(a) OVER () AS y FROM foo";
            assert_eq!(
                vec![r#"{"output":{"__bot._wf1":{"$sum":"$foo.a"}}}"#.to_string()],
                window_stages(sql)
            );
            assert_eq!(
                r#"{"__bot":{"x":"$__bot._wf1","y":"$__bot._wf1"},"_id":0}"#,
                final_projection(sql)
            );
        }

        // A user alias cannot capture a generated name: the generated names live under the
        // Bottom datasource and are only ever referenced by the expression built at hoist time.
        #[test]
        fn user_alias_colliding_with_the_generated_namespace() {
            assert_eq!(
                r#"{"__bot":{"_wf1":"$__bot._wf1"},"_id":0}"#,
                final_projection("SELECT SUM(a) OVER () AS _wf1 FROM foo")
            );
        }

        #[test]
        fn user_alias_colliding_with_a_source_column() {
            assert_eq!(
                r#"{"__bot":{"a":"$__bot._wf1"},"_id":0}"#,
                final_projection("SELECT SUM(a) OVER () AS a FROM foo")
            );
        }

        // An unaliased window function is named positionally by AddAliasRewritePass, so the
        // two aliasing mechanisms have to compose.
        #[test]
        fn unaliased_window_function_gets_a_positional_alias() {
            assert_eq!(
                r#"{"__bot":{"_1":"$__bot._wf1"},"_id":0}"#,
                final_projection("SELECT SUM(a) OVER () FROM foo")
            );
        }
    }

    // An aggregation nested inside a window argument is hoisted into the GROUP BY by
    // AggregateRewritePass, and the window then reads its output. The window's own callee is
    // a bare FunctionExpr, so it is never mistaken for a group aggregation.
    window_stages_test!(
        aggregate_nested_in_a_window_argument,
        sql = "SELECT SUM(SUM(a)) OVER () AS s FROM foo GROUP BY b AS b",
        expected = [r#"{"output":{"__bot._wf1":{"$sum":"$__bot._agg1"}}}"#]
    );

    // A plain aggregate does not gain a window stage, and a window function does not
    // synthesize an implicit GROUP BY.
    #[test]
    fn window_function_does_not_create_an_implicit_group() {
        let t = translate_sql(
            "test",
            "SELECT SUM(a) OVER () AS s FROM foo",
            &*CATALOG,
            SqlOptions::default(),
        )
        .unwrap();
        assert!(t
            .pipeline
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s.as_document().map(|d| !d.contains_key("$group")) != Some(false)));
    }

    /// Period-over-period comparison, the motivating use case: a value alongside its own
    /// lagged value and the delta between them.
    mod period_over_period {
        use super::*;

        const SQL: &str = "SELECT order_date, price, \
             LAG(price) OVER (ORDER BY order_date) AS one_day_before, \
             price - LAG(price) OVER (ORDER BY order_date) AS daily_change \
             FROM stock_price";

        // The same window call appears twice, once on its own and once inside an
        // arithmetic expression. De-duplication must collapse them to a single $shift.
        #[test]
        fn identical_window_calls_collapse_to_one_shift() {
            assert_eq!(
                vec![
                    r#"{"sortBy":{"stock_price.order_date":1},"output":{"__bot._wf1":{"$shift":{"output":"$stock_price.price","by":-1}}}}"#
                        .to_string()
                ],
                window_stages(SQL)
            );
        }

        // Both the bare reference and the subtraction read the same generated field.
        #[test]
        fn both_outputs_reference_the_same_generated_field() {
            assert_eq!(
                r#"{"__bot":{"order_date":"$stock_price.order_date","price":"$stock_price.price","one_day_before":"$__bot._wf1","daily_change":{"$subtract":["$stock_price.price","$__bot._wf1"]}},"_id":0}"#,
                final_projection(SQL)
            );
        }
    }

    error_test!(
        distinct_is_rejected,
        sql = "SELECT SUM(DISTINCT a) OVER () AS s FROM foo",
        expected = "algebrize error: Error 3036: DISTINCT is not supported in the window function `SUM(DISTINCT a) OVER ()`.\n\tCaused by:\n\tDISTINCT is not supported in a window function: SUM(DISTINCT a) OVER ()"
    );

    error_test!(
        rank_without_over_is_rejected,
        sql = "SELECT RANK() AS r FROM foo",
        expected = "algebrize error: Error 3037: `RANK()` is a window function and requires an OVER clause, for example `RANK() OVER (PARTITION BY ... ORDER BY ...)`.\n\tCaused by:\n\twindow function used without an OVER clause: RANK()"
    );

    error_test!(
        window_function_in_where_is_rejected,
        sql = "SELECT a AS a FROM foo WHERE SUM(a) OVER () > 1",
        expected = "rewrite error: window functions are not allowed in the WHERE clause"
    );

    error_test!(
        window_function_in_select_values_is_rejected,
        sql = "SELECT VALUE {'s': SUM(a) OVER ()} FROM foo",
        expected = "rewrite error: window functions are not allowed in a SELECT VALUE body"
    );

    error_test!(
        range_frame_without_order_by_is_rejected,
        sql =
            "SELECT SUM(a) OVER (RANGE BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW) AS s FROM foo",
        expected =
            "rewrite error: a RANGE window frame requires an ORDER BY in its window specification"
    );
}
