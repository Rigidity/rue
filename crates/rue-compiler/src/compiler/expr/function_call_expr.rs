use rue_typing::HashMap;

use rowan::TextRange;
use rue_parser::{AstNode, FunctionCallExpr};
use rue_typing::{deconstruct_items, unwrap_list, Callable, TypeId};

use crate::{compiler::Compiler, hir::Hir, value::Value, ErrorKind};

impl Compiler<'_> {
    pub fn compile_function_call_expr(&mut self, call: &FunctionCallExpr) -> Value {
        // Compile the callee expression.
        // We mark this expression as a callee to allow inline function references.
        self.is_callee = true;
        let callee = call.callee().map(|callee| self.compile_expr(&callee, None));

        // Get the function type of the callee.
        let function_type = callee
            .as_ref()
            .and_then(|callee| self.ty.get_callable_recursive(callee.type_id).cloned());

        let parameter_types = function_type.as_ref().map(|ty| {
            deconstruct_items(
                self.ty,
                ty.parameters,
                ty.parameter_names.len(),
                ty.nil_terminated,
            )
            .expect("invalid function type")
        });

        // Make sure the callee is callable, if present.
        if let Some(callee) = callee.as_ref() {
            if function_type.is_none() {
                self.db.error(
                    ErrorKind::UncallableType(self.type_name(callee.type_id)),
                    call.callee().unwrap().syntax().text_range(),
                );
            }
        }

        let generic_args = if let Some(generic_params) = function_type.as_ref().and_then(|fun| {
            if fun.generic_types.is_empty() {
                None
            } else {
                Some(fun.generic_types.clone())
            }
        }) {
            if let Some(generic_args) = call.generic_args() {
                let types = generic_args.types();

                if types.len() == generic_params.len() {
                    let mut generic_types = HashMap::new();

                    for (i, ty) in types.into_iter().enumerate() {
                        let type_id = self.compile_type(ty);
                        generic_types.insert(generic_params[i], type_id);
                    }

                    generic_types
                } else {
                    self.db.error(
                        ErrorKind::GenericArgsMismatch(types.len(), generic_params.len()),
                        generic_args.syntax().text_range(),
                    );

                    HashMap::new()
                }
            } else {
                HashMap::new()
            }
        } else {
            if let Some(generic_args) = call.generic_args() {
                self.db.error(
                    ErrorKind::UnexpectedGenericArgs,
                    generic_args.syntax().text_range(),
                );
            }
            HashMap::new()
        };

        // Push a generic type context for the function, and allow inference.
        self.generic_type_stack.push(generic_args);
        self.allow_generic_inference_stack.push(true);

        // Compile the arguments naively, and defer type checking until later.
        let mut args = Vec::new();

        let call_args = call.args();
        let len = call_args.len();
        let spread = call_args
            .last()
            .map(|arg| arg.spread().is_some())
            .unwrap_or(false);

        if let Some(function_type) = &function_type {
            self.check_argument_length(
                function_type,
                parameter_types.as_ref().unwrap(),
                len,
                call.syntax().text_range(),
            );
        }

        for (i, arg) in call_args.iter().enumerate() {
            // Determine the expected type.
            let expected_type = function_type.as_ref().and_then(|ty| {
                let parameter_types = parameter_types.as_ref().unwrap();

                if i < parameter_types.len() {
                    Some(parameter_types[i])
                } else if !ty.nil_terminated {
                    unwrap_list(self.ty, *parameter_types.last().unwrap())
                } else {
                    None
                }
            });

            // Compile the argument expression, if present.
            // Otherwise, it's a parser error
            let expr = arg
                .expr()
                .map(|expr| self.compile_expr(&expr, expected_type))
                .unwrap_or_else(|| self.unknown());

            // Add the argument to the list.
            let type_id = expr.type_id;
            args.push(expr);

            // Check if it's a spread argument.
            let last = i == len - 1;

            if arg.spread().is_some() && !last {
                self.db
                    .error(ErrorKind::InvalidSpreadArgument, arg.syntax().text_range());
            }

            // Check the type of the argument.
            let Some(function) = &function_type else {
                continue;
            };

            let parameter_types = parameter_types.as_ref().unwrap();

            if last && spread {
                if function.nil_terminated {
                    self.db.error(
                        ErrorKind::UnsupportedFunctionSpread,
                        call_args[i].syntax().text_range(),
                    );
                } else if i >= parameter_types.len() - 1 {
                    let expected_type = *parameter_types.last().unwrap();
                    self.type_check(type_id, expected_type, call_args[i].syntax().text_range());
                }
            } else if !function.nil_terminated && i >= parameter_types.len() - 1 {
                if let Some(inner_list_type) =
                    unwrap_list(self.ty, *parameter_types.last().unwrap())
                {
                    self.type_check(type_id, inner_list_type, call_args[i].syntax().text_range());
                } else if i == parameter_types.len() - 1 && !spread {
                    self.db.error(
                        ErrorKind::RequiredFunctionSpread,
                        call_args[i].syntax().text_range(),
                    );
                }
            } else if i < parameter_types.len() {
                let param_type = parameter_types[i];
                self.type_check(type_id, param_type, call_args[i].syntax().text_range());
            }
        }

        // The generic type context is no longer needed.
        let generic_types = self.generic_type_stack.pop().unwrap();
        self.allow_generic_inference_stack.pop().unwrap();

        // Calculate the return type.
        let mut type_id =
            function_type.map_or(self.ty.std().unknown, |expected| expected.return_type);

        if !generic_types.is_empty() {
            type_id = self.ty.substitute(type_id, generic_types);
        }

        // Build the HIR for the function call.
        let hir_id = self.db.alloc_hir(Hir::FunctionCall(
            callee.map_or(self.builtins.unknown, |callee| callee.hir_id),
            args.iter().map(|arg| arg.hir_id).collect(),
            spread,
        ));

        Value::new(hir_id, type_id)
    }

    fn check_argument_length(
        &mut self,
        function: &Callable,
        parameter_types: &[TypeId],
        length: usize,
        text_range: TextRange,
    ) {
        if function.nil_terminated {
            if length != parameter_types.len() {
                self.db.error(
                    ErrorKind::ArgumentMismatch(length, parameter_types.len()),
                    text_range,
                );
            }
        } else if unwrap_list(self.ty, *parameter_types.last().unwrap()).is_some() {
            if length < parameter_types.len() - 1 {
                self.db.error(
                    ErrorKind::ArgumentMismatchSpread(length, parameter_types.len()),
                    text_range,
                );
            }
        } else if length != parameter_types.len() {
            self.db.error(
                ErrorKind::ArgumentMismatch(length, parameter_types.len()),
                text_range,
            );
        }
    }
}
