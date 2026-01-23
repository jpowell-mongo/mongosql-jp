use crate::air;
use crate::air::desugarer::Pass;
use crate::air::util::sql_op_to_mql_op;
use crate::air::visitor::Visitor;
use crate::air::{
    Expression, Expression::*, Match, MatchLanguage, MqlSemanticOperator, SqlOperator,
    SqlSemanticOperator, Stage,
};

pub struct OrExpressionsDesugarerPass;


/// For Match stages, rewrites any SqlOperator::Or expressions into MqlOperator expressions.
/// Any Sql Semantic Operators that are descendants of the Or expression will also be translated to MqlOperator expressions.
///
/// The MatchStageDesugarerVisitor is responsible for "finding" all the Match Stages.
/// Then, the OrExpressionsDesugarerVisitor is responsible for walking the Match condition and rewriting
/// the SqlOperator::Or expressions (and its descendants) into MqlOperator expressions.
impl Pass for OrExpressionsDesugarerPass {
    fn apply(&self, pipeline: Stage) -> air::desugarer::Result<Stage> {
        let visitor = &mut MatchStageDesugarerVisitor {};
        let stage = pipeline.walk(visitor);
        Ok(stage)
    }
}

#[derive(Default)]
struct MatchStageDesugarerVisitor {}

impl MatchStageDesugarerVisitor {}
impl Visitor for MatchStageDesugarerVisitor {
    fn visit_match(&mut self, node: Match) -> Match {
        let node: Match = match node {
            Match::MatchLanguage(m) => Match::MatchLanguage(MatchLanguage {
                source: m.source,
                expr: m.expr,
            }),
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
        match node {
            SqlSemanticOperator(s) => match s.op {
                SqlOperator::Or => {
                    let old_is_within_or_context = self.is_within_or_context;
                    self.is_within_or_context = true;
                    let desugared_parent = self.desugar_sql_semantic_operator_expression(s);
                    let desugared_tree = desugared_parent.walk(self);
                    self.is_within_or_context = old_is_within_or_context;
                    desugared_tree
                }
                _ => {
                    let desugared_parent = self.desugar_sql_semantic_operator_expression(s);
                    desugared_parent.walk(self)
                }
            },
            _ => node,
        }
    }
}
