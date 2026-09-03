use crate::ast::{
    self,
    rewrites::{Error, Pass, Result},
    visitor::Visitor,
};

/// Validates window function usage that the grammar cannot check on its own.
///
/// The grammar rejects malformed frames locally (see `parse_window_frame`), but two rules
/// need surrounding context: which clause a window function appears in, and whether a
/// `RANGE` frame has a sort to be measured against.
pub struct WindowFunctionsRewritePass;

impl Pass for WindowFunctionsRewritePass {
    fn apply(&self, query: ast::Query) -> Result<ast::Query> {
        let mut visitor = WindowFunctionsRewriteVisitor::default();
        let rewritten = query.walk(&mut visitor);
        match visitor.error {
            Some(err) => Err(err),
            None => Ok(rewritten),
        }
    }
}

struct WindowFunctionsRewriteVisitor {
    /// The clause currently being walked, used both to decide whether a window function is
    /// legal here and to name the offending clause in the error.
    clause: &'static str,
    /// Whether we are inside a `SELECT VALUE` body, where window functions are rejected for
    /// the same reason aggregations are.
    in_select_values: bool,
    error: Option<Error>,
}

impl Default for WindowFunctionsRewriteVisitor {
    fn default() -> Self {
        Self {
            clause: SELECT,
            in_select_values: false,
            error: None,
        }
    }
}

/// The only clause where a window function is legal. WHERE, GROUP BY and HAVING all run
/// before windowing, so they cannot refer to a window function. ORDER BY runs after, but
/// we do not support window functions there either.
const SELECT: &str = "SELECT";

impl WindowFunctionsRewriteVisitor {
    fn window_allowed(&self) -> bool {
        self.clause == SELECT && !self.in_select_values
    }

    fn set_error(&mut self, err: Error) {
        // Keep the first error, so the message points at the earliest problem.
        if self.error.is_none() {
            self.error = Some(err);
        }
    }
}

impl Visitor for WindowFunctionsRewriteVisitor {
    fn visit_select_query(&mut self, node: ast::SelectQuery) -> ast::SelectQuery {
        let ast::SelectQuery {
            select_clause,
            from_clause,
            where_clause,
            group_by_clause,
            having_clause,
            order_by_clause,
            limit,
            offset,
        } = node;

        // `clause` is set immediately before each walk rather than once per group, so a
        // nested subquery restoring it to its own last value cannot leak into our next
        // clause.
        self.clause = SELECT;
        let select_clause = select_clause.walk(self);

        self.clause = "ORDER BY";
        let order_by_clause = order_by_clause.map(|o| o.walk(self));

        self.clause = "FROM";
        let from_clause = from_clause.map(|f| f.walk(self));

        self.clause = "WHERE";
        let where_clause = where_clause.map(|w| w.walk(self));

        self.clause = "GROUP BY";
        let group_by_clause = group_by_clause.map(|g| g.walk(self));

        self.clause = "HAVING";
        let having_clause = having_clause.map(|h| h.walk(self));

        ast::SelectQuery {
            select_clause,
            from_clause,
            where_clause,
            group_by_clause,
            having_clause,
            order_by_clause,
            limit,
            offset,
        }
    }

    fn visit_select_body(&mut self, node: ast::SelectBody) -> ast::SelectBody {
        // Mirrors `AggregateAliasingVisitor::visit_select_body`: track whether we are inside
        // a SELECT VALUE body, saving and restoring so nested queries do not leak state.
        let was_in_select_values = self.in_select_values;
        self.in_select_values = matches!(node, ast::SelectBody::Values(_));
        let node = node.walk(self);
        self.in_select_values = was_in_select_values;
        node
    }

    fn visit_expression(&mut self, node: ast::Expression) -> ast::Expression {
        if matches!(node, ast::Expression::Window(_)) && !self.window_allowed() {
            self.set_error(if self.in_select_values {
                Error::WindowFunctionInSelectValues
            } else {
                Error::WindowFunctionNotAllowedInClause(self.clause)
            });
            return node;
        }
        node.walk(self)
    }

    fn visit_window_spec(&mut self, node: ast::WindowSpec) -> ast::WindowSpec {
        // A RANGE frame measures offsets against the sort key's value, so without an
        // ORDER BY there is nothing to measure. This mirrors the MQL restriction that a
        // range window needs exactly one ascending sort key.
        if node.order_by.is_empty()
            && matches!(
                node.frame,
                Some(ast::WindowFrame {
                    units: ast::WindowFrameUnits::Range,
                    ..
                })
            )
        {
            self.set_error(Error::RangeWindowRequiresOrderBy);
        }
        node.walk(self)
    }
}
