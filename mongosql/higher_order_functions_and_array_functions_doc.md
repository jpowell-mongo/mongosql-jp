# Higher Order Functions and Array Functions

MongoSQL provides three **higher order functions** — `MAP`, `FILTER`, and `REDUCE` — that apply an
expression to the elements of an array, plus eleven **`ARRAY_*` functions** that are shorthand for
common higher order function patterns.

Every `ARRAY_*` function is pure syntactic sugar: it is rewritten into an equivalent `MAP`,
`FILTER`, or `REDUCE` expression before schema checking, in
`mongosql/src/ast/rewrites/higher_order_functions.rs`. There is no separate runtime implementation.

## Function list

| Function | Arity | Summary |
| --- | --- | --- |
| `MAP(array, f)` | 2 | Apply an expression to every element; returns an array of results. |
| `FILTER(array, f)` | 2 | Keep elements for which the predicate is `TRUE`. |
| `REDUCE(array, init, f)` | 3 | Fold the array into a single value, starting from `init`. |
| `ARRAY_CAST(array, type)` | 2 | Cast every element to the target type. |
| `ARRAY_EXTRACT(array, path)` | 2 | Pull a field path out of every element. |
| `ARRAY_COMPACT(array)` | 1 | Drop `NULL` and missing elements. |
| `ARRAY_REMOVE(array, x)` | 2 | Drop elements equal to `x`. |
| `ARRAY_COUNT_IF(array, f)` | 2 | Count elements satisfying a predicate. |
| `ARRAY_SUM(array)` | 1 | Sum the elements. |
| `ARRAY_PRODUCT(array)` | 1 | Multiply the elements. |
| `ARRAY_AVG(array)` | 1 | Mean of the elements. |
| `ARRAY_ALL(array)` | 1 | Conjunction of an array of booleans. |
| `ARRAY_ANY(array)` | 1 | Disjunction of an array of booleans. |
| `ARRAY_JOIN(array [, sep])` | 1 or 2 | Concatenate an array of strings. |

### Desugaring

The Array functions are light wrappers around the higher order functions. This table
demonstrates the equivalent functions using MAP / FILTER / REDUCE.

| Function | Rewritten as |
| --- | --- |
| `ARRAY_CAST(a, T)` | `MAP(a, CAST(this AS T))` |
| `ARRAY_EXTRACT(a, x.y)` | `MAP(a, this.x.y)` |
| `ARRAY_COMPACT(a)` | `FILTER(a, NOT this IS NULL)` |
| `ARRAY_REMOVE(a, x)` | `FILTER(a, this <> x)` |
| `ARRAY_COUNT_IF(a, f)` | `SIZE(FILTER(a, f))` |
| `ARRAY_SUM(a)` | ``REDUCE(a, 0, `value` + this)`` |
| `ARRAY_PRODUCT(a)` | ``REDUCE(a, 1, `value` * this)`` |
| `ARRAY_AVG(a)` | ``REDUCE(a, 0, `value` + this) / SIZE(a)`` |
| `ARRAY_ALL(a)` | ``REDUCE(a, TRUE, `value` AND this)`` |
| `ARRAY_ANY(a)` | ``REDUCE(a, FALSE, `value` OR this)`` |
| `ARRAY_JOIN(a)` | ``REDUCE(a, '', `value` \|\| this)`` |
| `ARRAY_JOIN(a, sep)` | ``TRIM(LEADING sep FROM REDUCE(a, '', `value` \|\| sep \|\| this))`` |

## The `this` and `` `value` `` variables

Inside the body of a higher order function:

| Variable | Available in | Meaning |
| --- | --- | --- |
| `this` | `MAP`, `FILTER`, `REDUCE` | The current array element. |
| `` `value` `` | `REDUCE` | The accumulated result so far. |

`` `value` `` must be written with backticks because `VALUE` is a reserved keyword.

These map directly onto MQL's native `$$this` and `$$value` — the translator never emits an `as`
name for `$map`/`$filter`, so the generated aggregation uses the built-in variables
(`mongosql/src/codegen/expressions.rs`).

**Shadowing.** An unqualified `this` or `value` inside a higher order function body always resolves
to the variable, shadowing any document field of the same name
(`algebrize_unqualified_identifier`, `mongosql/src/algebrizer/definitions.rs`). To read a document
field actually named `this`, qualify it with the datasource:

```sql
-- `this` is the element; `t.this` is the document field named "this"
SELECT MAP(a, this + t.this) FROM t AS t
```

This behavior is covered by the "There is no ambiguity" cases in
`tests/spec_tests/query_tests/higher_order_functions.yml`.

Bodies may freely reference fields from the enclosing document:

```sql
SELECT sensor, MAP(measurements, this + CHAR_LENGTH(sensor)) AS bumped
FROM readings WHERE sensor = 'c'
```

| `sensor` | `bumped` |
| --- | --- |
| `"c"` | `[4, 13, 21]` |

### Point-free shorthand

The function argument may be a bare operator or function name. In `MAP`/`FILTER` it is applied to
`this`; in `REDUCE` it is applied to `` `value` `` and `this`.

```sql
SELECT MAP(measurements, ABS)     -- same as MAP(measurements, ABS(this))
SELECT REDUCE(measurements, 0, +) -- same as REDUCE(measurements, 0, `value` + this)
```

Both were verified to return `[3, 12, 20]` and `35` respectively for sensor `c`.

## Null, missing, and empty-array behavior

- If the array argument is `NULL` **or missing**, the whole expression returns `NULL`.
- A `NULL` *element* propagates according to the body expression's SQL null semantics. `MAP` yields
  `NULL` in that position; `REDUCE` typically collapses the entire result to `NULL`.
- `FILTER` keeps an element only when the predicate is `TRUE`. A predicate that evaluates to `NULL`
  excludes the element, which is why `FILTER` also removes `NULL` elements.
- Empty arrays return the identity value: `[]` for `MAP`/`FILTER`, `init` for `REDUCE`, `0` for
  `ARRAY_SUM`, `1` for `ARRAY_PRODUCT`, `TRUE` for `ARRAY_ALL`, `FALSE` for `ARRAY_ANY`, `''` for
  `ARRAY_JOIN` — but `NULL` for `ARRAY_AVG` (it divides by `SIZE(a)`).

No `$ifNull` guard is generated; MongoSQL relies on MQL's own nullish handling plus static
nullability tracking in `mongosql/src/mir/schema/mod.rs`.

## Sample dataset

All examples below were executed against a live `mongod` (8.3.4) at `mongodb://localhost:27017`,
database `sql_higher_order_functions`, collection `readings`:

```javascript
db.readings.insertMany([
  { _id: 1, sensor: "a", measurements: [],            samples: [] },
  { _id: 2, sensor: "b", measurements: [7],           samples: [{ reading: { amount: 7 } }] },
  { _id: 3, sensor: "c", measurements: [3, 12, 20],   samples: [{ reading: { amount: 3 } }, { reading: { amount: 12 } }, { reading: { amount: 20 } }] },
  { _id: 4, sensor: "d", measurements: [5, null, 18], samples: [{ reading: { amount: 5 } }, null, { reading: { amount: 18 } }] },
  { _id: 5, sensor: "e", measurements: null,          samples: null },
  { _id: 6, sensor: "f" }
])
```

The six documents cover the four nullish cases the spec tests exercise: empty array (`a`), a `NULL`
element (`d`), a `NULL` array (`e`), and a missing array (`f`).

Note the field is named `measurements`, not `values`, and the subfield `amount`, not `value` —
`VALUES` and `VALUE` are reserved keywords and would otherwise require backticks throughout.

## Higher order functions

### MAP

Applies an expression to each element and returns an array of the results.

```sql
SELECT sensor, MAP(measurements, this + 1) AS incremented FROM readings
```

| `sensor` | `incremented` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `[8]` |
| `"c"` | `[4, 13, 21]` |
| `"d"` | `[6, null, 19]` |
| `"e"` | `null` |
| `"f"` | `null` |

### FILTER

Returns the elements for which the predicate is `TRUE`. Note that sensor `d`'s `NULL` element is
dropped, because `null > 10` is `NULL`, not `TRUE`.

```sql
SELECT sensor, FILTER(measurements, this > 10) AS high FROM readings
```

| `sensor` | `high` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `[]` |
| `"c"` | `[12, 20]` |
| `"d"` | `[18]` |
| `"e"` | `null` |
| `"f"` | `null` |

### REDUCE

Folds the array into a single value. `` `value` `` starts at the initial value and carries the
accumulated result.

```sql
SELECT sensor, REDUCE(measurements, 0, `value` + this) AS total FROM readings
```

| `sensor` | `total` |
| --- | --- |
| `"a"` | `0` |
| `"b"` | `7` |
| `"c"` | `35` |
| `"d"` | `null` |
| `"e"` | `null` |
| `"f"` | `null` |

The empty array returns the initial value unchanged, and any `NULL` element poisons the whole
accumulation. A non-zero initial value works as expected — with `100`, sensor `c` returns `135`.

`REDUCE` is not limited to arithmetic; the accumulator can be any type:

```sql
SELECT REDUCE(measurements, '', `value` || CAST(this AS STRING)) AS concat
FROM readings WHERE sensor = 'c'
```

returns `"31220"`.

## Array functions

### ARRAY_CAST

Casts every element to the target type.

```sql
SELECT sensor, ARRAY_CAST(measurements, STRING) AS labels FROM readings
```

| `sensor` | `labels` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `["7"]` |
| `"c"` | `["3", "12", "20"]` |
| `"d"` | `["5", null, "18"]` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_EXTRACT

Extracts a field path from every element of an array of documents. Missing or `NULL` elements
produce `NULL`.

```sql
SELECT sensor, ARRAY_EXTRACT(samples, reading.amount) AS extracted FROM readings
```

| `sensor` | `extracted` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `[7]` |
| `"c"` | `[3, 12, 20]` |
| `"d"` | `[5, null, 18]` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_COMPACT

Removes `NULL` and missing elements.

```sql
SELECT sensor, ARRAY_COMPACT(measurements) AS compacted FROM readings
```

| `sensor` | `compacted` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `[7]` |
| `"c"` | `[3, 12, 20]` |
| `"d"` | `[5, 18]` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_REMOVE

Removes elements equal to the second argument.

```sql
SELECT sensor, ARRAY_REMOVE(measurements, 5) AS removed FROM readings
```

| `sensor` | `removed` |
| --- | --- |
| `"a"` | `[]` |
| `"b"` | `[7]` |
| `"c"` | `[3, 12, 20]` |
| `"d"` | `[18]` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_COUNT_IF

Counts the elements satisfying a predicate.

```sql
SELECT sensor, ARRAY_COUNT_IF(measurements, this > 10) AS high_count FROM readings
```

| `sensor` | `high_count` |
| --- | --- |
| `"a"` | `0` |
| `"b"` | `0` |
| `"c"` | `2` |
| `"d"` | `1` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_SUM

Sums the elements. Returns `0` for an empty array and `NULL` if any element is `NULL`.

```sql
SELECT sensor, ARRAY_SUM(measurements) AS total FROM readings
```

| `sensor` | `total` |
| --- | --- |
| `"a"` | `0` |
| `"b"` | `7` |
| `"c"` | `35` |
| `"d"` | `null` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_PRODUCT

Multiplies the elements. Returns `1` for an empty array.

```sql
SELECT sensor, ARRAY_PRODUCT(measurements) AS product FROM readings
```

| `sensor` | `product` |
| --- | --- |
| `"a"` | `1` |
| `"b"` | `7` |
| `"c"` | `720` |
| `"d"` | `null` |
| `"e"` | `null` |
| `"f"` | `null` |

### ARRAY_AVG

Mean of the elements. Returns `NULL` for an empty array, since it divides by `SIZE(a)`.

```sql
SELECT sensor, ARRAY_AVG(measurements) AS average FROM readings
```

| `sensor` | `average` |
| --- | --- |
| `"a"` | `null` |
| `"b"` | `7` |
| `"c"` | `11` |
| `"d"` | `null` |
| `"e"` | `null` |
| `"f"` | `null` |

> **Known discrepancy.** Sensor `c` is `[3, 12, 20]`, whose true mean is `11.666…`, but the query
> returns `11`. Because both operands of the desugared `REDUCE(...) / SIZE(a)` are integers, the
> inferred result type is integer and codegen wraps the `$divide` in
> `{"$convert": {..., "to": "int"}}`, truncating the fractional part. Casting first gives the
> correct answer:
>
> ```sql
> SELECT ARRAY_AVG(ARRAY_CAST(measurements, DOUBLE)) FROM readings WHERE sensor = 'c'
> ```
>
> returns `11.666666666666666`. The existing `ARRAY_AVG` spec tests do not catch this because their
> inputs (`[1]`, `[1, 2, 3]`) have exact integer means.

### ARRAY_ALL and ARRAY_ANY

Conjunction and disjunction of an array of booleans, following SQL three-valued logic. Since the
sample dataset has no boolean arrays, these examples build one with `MAP`.

```sql
SELECT sensor, ARRAY_ALL(MAP(measurements, this > 4)) AS all_gt4 FROM readings
```

| `sensor` | `all_gt4` |
| --- | --- |
| `"a"` | `true` |
| `"b"` | `true` |
| `"c"` | `false` |
| `"d"` | `null` |
| `"e"` | `null` |
| `"f"` | `null` |

```sql
SELECT sensor, ARRAY_ANY(MAP(measurements, this > 15)) AS any_gt15 FROM readings
```

| `sensor` | `any_gt15` |
| --- | --- |
| `"a"` | `false` |
| `"b"` | `false` |
| `"c"` | `true` |
| `"d"` | `true` |
| `"e"` | `null` |
| `"f"` | `null` |

Sensor `d` shows the three-valued logic clearly: its mapped array is `[false, null, true]`, so
`ARRAY_ALL` is `NULL` (undetermined) while `ARRAY_ANY` is `TRUE` (one `TRUE` is decisive).

### ARRAY_JOIN

Concatenates an array of strings, with an optional separator.

```sql
SELECT sensor, ARRAY_JOIN(ARRAY_CAST(ARRAY_COMPACT(measurements), STRING), ', ') AS joined
FROM readings
```

| `sensor` | `joined` |
| --- | --- |
| `"a"` | `""` |
| `"b"` | `"7"` |
| `"c"` | `"3, 12, 20"` |
| `"d"` | `"5, 18"` |
| `"e"` | `null` |
| `"f"` | `null` |

Without a separator the same query returns `"31220"` for sensor `c`. A `NULL` element yields `NULL`
for the whole result, which is why `ARRAY_COMPACT` is applied first here.

## Composition

These functions nest freely and can be used anywhere a scalar expression is allowed, including
`WHERE`:

```sql
SELECT sensor, ARRAY_SUM(FILTER(measurements, this > 4)) AS sum_gt4 FROM readings
```

| `sensor` | `sum_gt4` |
| --- | --- |
| `"a"` | `0` |
| `"b"` | `7` |
| `"c"` | `32` |
| `"d"` | `23` |
| `"e"` | `null` |
| `"f"` | `null` |

Sensor `d` returns `23` rather than `NULL`: `FILTER` drops the `NULL` element first, so
`ARRAY_SUM` never sees it. Filtering before aggregating is the idiomatic way to make these
functions `NULL`-tolerant.

```sql
SELECT sensor FROM readings WHERE ARRAY_ANY(MAP(measurements, this > 15))
```

returns sensors `c` and `d`.

## Reproducing these results

```bash
cargo build --release -p mongosql-cli
mongosh "mongodb://localhost:27017/sql_higher_order_functions" --eval '<insertMany from above>'
./target/release/mongosql-cli -d sql_higher_order_functions -f <schema.yml> -e "<query>"
```

The CLI's `-f` flag takes a `{db: {collection: <json schema>}}` catalog file. Without it, the CLI
looks for schemas in the `__sql_schemas` collection and falls back to an empty relaxed-mode schema,
which still runs but yields less precise result-set types.
