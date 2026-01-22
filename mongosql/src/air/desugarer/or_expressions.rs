use crate::air;
use crate::air::desugarer::Pass;
use crate::air::util::sql_op_to_mql_op;
use crate::air::visitor::Visitor;
use crate::air::{
    Expression, Expression::*, Match, MatchLanguage, MqlSemanticOperator, SqlOperator,
    SqlSemanticOperator, Stage,
};

pub struct OrExpressionsDesugarerPass;

impl Pass for OrExpressionsDesugarerPass {
    fn apply(&self, pipeline: Stage) -> air::desugarer::Result<Stage> {
        let visitor = &mut FilterStageDesugarerVisitor {};
        let stage = pipeline.walk(visitor);
        Ok(stage)
    }
}

#[derive(Default)]
struct FilterStageDesugarerVisitor {}

impl FilterStageDesugarerVisitor {}
impl Visitor for FilterStageDesugarerVisitor {
    fn visit_match(&mut self, node: Match) -> Match {
        let node: Match = match node {
            Match::MatchLanguage(m) => {
                let or_expression_visitor = &mut OrExpressionsDesugarerVisitor::default();
                let visited = m.expr.walk(or_expression_visitor);
                Match::MatchLanguage(MatchLanguage {
                    source: m.source,
                    expr: Box::new(visited),
                })
            }
            Match::ExprLanguage(e) => {
                let or_expression_visitor = &mut OrExpressionsDesugarerVisitor::default();
                let visited = e.walk(or_expression_visitor);
                Match::ExprLanguage(visited)
            }
        };
        node.walk(self)
    }
}

#[derive(Default)]
struct OrExpressionsDesugarerVisitor {
    is_within_or_context: bool,
}

impl OrExpressionsDesugarerVisitor {
    fn desugar_sql_semantic_operator_expression(
        &self,
        operator: SqlSemanticOperator,
    ) -> Expression {
        let as_mql_op = sql_op_to_mql_op(operator.op);
        if !self.is_within_or_context || as_mql_op.is_none() {
            return SqlSemanticOperator(operator);
        }

        MqlSemanticOperator(MqlSemanticOperator {
            op: as_mql_op.unwrap(),
            args: operator.args,
        })
    }
}

impl Visitor for OrExpressionsDesugarerVisitor {
    fn visit_expression(&mut self, node: Expression) -> Expression {
        let node = match node {
            SqlSemanticOperator(s) => match s.op {
                SqlOperator::Or => {
                    self.is_within_or_context = true;
                    let desugared_parent = self.desugar_sql_semantic_operator_expression(s);
                    let desugared_tree = desugared_parent.walk(self);
                    self.is_within_or_context = false;
                    desugared_tree
                }
                _ => {
                    let desugared_parent = self.desugar_sql_semantic_operator_expression(s);
                    desugared_parent.walk(self)
                }
            },
            // Add SubqueryComparison and Exists here
            Subquery(s) => {
                self.is_within_or_context = false;
                Subquery(s)
            }
            SubqueryComparison(s) => {
                self.is_within_or_context = false;
                SubqueryComparison(s)
            }
            // Note: I couldn't seem to get exists operator to match here?
            SubqueryExists(s) => {
                self.is_within_or_context = false;
                SubqueryExists(s)
            }

            _ => node,
        };

        node.walk(self)
    }
}
