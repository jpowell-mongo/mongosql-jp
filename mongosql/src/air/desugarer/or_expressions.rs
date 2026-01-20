use crate::air;
use crate::air::desugarer::Pass;
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
            in_or_context: false,
        };
        let stage = pipeline.walk(visitor);
        Ok(stage)
    }
}

#[derive(Default)]
struct OrExpressionsDesugarerVisitor {
    in_or_context: bool,
}

impl OrExpressionsDesugarerVisitor {
    fn desugar_or_expression(&self, operator: SqlSemanticOperator) -> Expression {
        MqlSemanticOperator(MqlSemanticOperator {
            op: MqlOperator::Or,
            args: operator.args,
        })
    }
}

impl Visitor for OrExpressionsDesugarerVisitor {
    fn visit_expression(&mut self, node: Expression) -> Expression {
        let node = match node {
            SqlSemanticOperator(s) => match s.op {
                SqlOperator::Or => {
                    self.in_or_context = true;
                    self.desugar_or_expression(s)
                }
                _ => SqlSemanticOperator(s),
            },
            Subquery(s) => {
                self.in_or_context = false;
                Subquery(s)
            }
            _ => node,
        };

        node.walk(self)
    }
}
