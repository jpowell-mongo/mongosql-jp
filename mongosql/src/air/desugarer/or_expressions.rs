use crate::air;
use crate::air::desugarer::Pass;
use crate::air::util::sql_op_to_mql_op;
use crate::air::visitor::Visitor;
use crate::air::{
    Expression, Expression::*, MqlOperator, MqlSemanticOperator, SqlOperator, SqlSemanticOperator,
    Stage,
};

// Notes: Do I want to keep track of the scope level as I traverse the tree?
pub struct OrExpressionsDesugarerPass;

impl Pass for OrExpressionsDesugarerPass {
    fn apply(&self, pipeline: Stage) -> air::desugarer::Result<Stage> {
        let visitor = &mut OrExpressionsDesugarerVisitor {
            is_within_or_context: false,
        };
        let stage = pipeline.walk(visitor);
        Ok(stage)
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
        // TODO: We want to re-write ALL sqlSemantic operators to mqlSemantic operators that are children in an or tree.
        let node = match node {
            SqlSemanticOperator(s) => match s.op {
                SqlOperator::Or => {
                    self.is_within_or_context = true;
                    let n = self.desugar_sql_semantic_operator_expression(s);
                    let completed_tree = n.walk(self);
                    self.is_within_or_context = false;
                    completed_tree
                }
                _ => {
                    let n = self.desugar_sql_semantic_operator_expression(s);
                    let completed_tree = n.walk(self);
                    completed_tree
                }
            },
            // Add SubqueryComparison and Exists here
            Subquery(s) => {
                self.is_within_or_context = false;
                Subquery(s)
            }
            _ => node,
        };

        node.walk(self)
    }
}
