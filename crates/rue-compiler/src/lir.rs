use crate::database::LirId;

#[derive(Debug, Clone)]
pub enum Lir {
    Atom(Vec<u8>),
    List(Vec<LirId>),
    Path(u32),
    Run(LirId, Vec<LirId>),
    Curry(LirId, Vec<LirId>),
    Closure(LirId, Vec<LirId>),
    FunctionBody(LirId),
    First(LirId),
    Rest(LirId),
    If(LirId, LirId, LirId),
    Not(LirId),
    Any(Vec<LirId>),
    Add(Vec<LirId>),
    Sub(Vec<LirId>),
    Mul(Vec<LirId>),
    Div(LirId, LirId),
    Divmod(LirId, LirId),
    Eq(LirId, LirId),
    Gt(LirId, LirId),
}
