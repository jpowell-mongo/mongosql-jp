use crate::{
    map,
    mir::{schema::SchemaCache, *},
    schema::{Atomic, Document, ResultSet, Schema, ANY_DOCUMENT},
    set, test_schema,
    util::mir_collection,
};
use agg_ast::definitions::Namespace;
use mongosql_datastructures::binding_tuple::Key;

fn window(functions: Vec<AliasedWindowFunction>) -> Stage {
    Stage::Window(Window {
        source: mir_collection("db", "foo"),
        partition_by: vec![],
        sort_by: vec![],
        functions,
        cache: SchemaCache::new(),
        scope: 0,
    })
}

fn sum_of_a(frame: Option<WindowFrame>) -> WindowExpr {
    WindowExpr::Aggregation(WindowAggregation {
        function: AggregationFunction::Sum,
        arg: Box::new(Expression::FieldAccess(FieldAccess::new(
            Box::new(Expression::Reference(("foo", 0u16).into())),
            "a".to_string(),
        ))),
        frame,
    })
}

// Unlike Group, a Window stage preserves its input bindings and only appends. The source
// datasource must still be present alongside the new field under Bottom.
test_schema!(
    window_schema_is_additive,
    expected = Ok(ResultSet {
        schema_env: map! {
            ("foo", 0u16).into() => ANY_DOCUMENT.clone(),
            Key::bot(0) => Schema::Document(Document {
                keys: map! { "_wf1".into() => Schema::AnyOf(set! {
                    Schema::Atomic(Atomic::Integer),
                    Schema::Atomic(Atomic::Long),
                }) },
                required: set! { "_wf1".into() },
                additional_properties: false,
                ..Default::default()
            }),
        },
        min_size: 0,
        max_size: None,
    }),
    input = window(vec![AliasedWindowFunction {
        alias: "_wf1".to_string(),
        window_expr: WindowExpr::CountStar,
    }]),
    catalog = Catalog::new(map! {
        Namespace {database: "db".into(), collection: "foo".into()} => ANY_DOCUMENT.clone(),
    }),
);

// A Window whose source already binds Bottom merges into that document rather than
// replacing it, which is what lets a window sit on top of a GROUP BY.
test_schema!(
    window_merges_into_an_existing_bottom_datasource,
    expected = Ok(ResultSet {
        schema_env: map! {
            ("foo", 0u16).into() => ANY_DOCUMENT.clone(),
            Key::bot(0) => Schema::Document(Document {
                keys: map! {
                    "_agg1".into() => Schema::Atomic(Atomic::Integer),
                    "_wf1".into() => Schema::AnyOf(set! {
                        Schema::Atomic(Atomic::Integer),
                        Schema::Atomic(Atomic::Long),
                    }),
                },
                required: set! { "_agg1".into(), "_wf1".into() },
                additional_properties: false,
                ..Default::default()
            }),
        },
        min_size: 0,
        max_size: None,
    }),
    input = Stage::Window(Window {
        source: Box::new(Stage::Project(Project {
            is_add_fields: true,
            source: mir_collection("db", "foo"),
            expression: map! {
                Key::bot(0) => Expression::Document(
                    crate::unchecked_unique_linked_hash_map! {
                        "_agg1".into() => Expression::Literal(LiteralValue::Integer(1))
                    }
                    .into()
                ),
            },
            cache: SchemaCache::new(),
        })),
        partition_by: vec![],
        sort_by: vec![],
        functions: vec![AliasedWindowFunction {
            alias: "_wf1".to_string(),
            window_expr: WindowExpr::CountStar,
        }],
        cache: SchemaCache::new(),
        scope: 0,
    }),
    catalog = Catalog::new(map! {
        Namespace {database: "db".into(), collection: "foo".into()} => ANY_DOCUMENT.clone(),
    }),
);

// An unbounded window always has rows to aggregate, so the accumulator schema is exact.
test_schema!(
    unbounded_frame_is_not_nullable,
    expected_pat = Ok(ResultSet { .. }),
    input = window(vec![AliasedWindowFunction {
        alias: "_wf1".to_string(),
        window_expr: sum_of_a(None),
    }]),
    catalog = Catalog::new(map! {
        Namespace {database: "db".into(), collection: "foo".into()} => Schema::Document(Document {
            keys: map! { "a".into() => Schema::Atomic(Atomic::Integer) },
            required: set! { "a".into() },
            additional_properties: false,
            ..Default::default()
        }),
    }),
);

// A bounded frame can be empty near a partition edge, so the result becomes nullable.
#[test]
fn bounded_frame_is_nullable() {
    use crate::{catalog::Catalog, mir::schema::SchemaInferenceState, CachedSchema, SchemaCheckingMode};

    let catalog = Catalog::new(map! {
        Namespace {database: "db".into(), collection: "foo".into()} => Schema::Document(Document {
            keys: map! { "a".into() => Schema::Atomic(Atomic::Integer) },
            required: set! { "a".into() },
            additional_properties: false,
            ..Default::default()
        }),
    });
    let frame = WindowFrame {
        units: WindowFrameUnits::Documents,
        bounds: WindowRange {
            lower: WindowBoundary::Position(-3),
            upper: WindowBoundary::Position(-2),
        },
    };
    let bounded = window(vec![AliasedWindowFunction {
        alias: "_wf1".to_string(),
        window_expr: sum_of_a(Some(frame)),
    }]);
    let unbounded = window(vec![AliasedWindowFunction {
        alias: "_wf1".to_string(),
        window_expr: sum_of_a(None),
    }]);

    let schema_of = |stage: Stage| {
        let state = SchemaInferenceState::new(
            0u16,
            crate::SchemaEnvironment::default(),
            &catalog,
            map! {},
            SchemaCheckingMode::Strict,
        );
        stage
            .schema(&state)
            .unwrap()
            .schema_env
            .get(&Key::bot(0))
            .unwrap()
            .clone()
    };

    let nullable = schema_of(bounded);
    let exact = schema_of(unbounded);
    assert_ne!(
        exact, nullable,
        "a bounded frame should widen the schema relative to an unbounded one"
    );
    assert!(
        format!("{nullable:?}").contains("Null"),
        "expected a bounded frame to be nullable, got {nullable:?}"
    );
    assert!(
        !format!("{exact:?}").contains("Null"),
        "expected an unbounded frame to stay non-null, got {exact:?}"
    );
}
