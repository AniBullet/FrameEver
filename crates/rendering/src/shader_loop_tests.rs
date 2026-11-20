use wgpu::naga::{
    Block, Expression, Function, Handle, Module, SampleLevel, Statement, front::wgsl,
};

fn takes_screen_derivative(expression: &Expression) -> bool {
    matches!(
        expression,
        Expression::Derivative { .. }
            | Expression::ImageSample {
                level: SampleLevel::Auto | SampleLevel::Bias(_),
                ..
            }
    )
}

fn calls_any(block: &Block, callees: &[Handle<Function>]) -> bool {
    block.iter().any(|statement| match statement {
        Statement::Call { function, .. } => callees.contains(function),
        Statement::Block(inner) => calls_any(inner, callees),
        Statement::If { accept, reject, .. } => {
            calls_any(accept, callees) || calls_any(reject, callees)
        }
        Statement::Switch { cases, .. } => cases.iter().any(|case| calls_any(&case.body, callees)),
        Statement::Loop {
            body, continuing, ..
        } => calls_any(body, callees) || calls_any(continuing, callees),
        _ => false,
    })
}

fn derivative_functions(module: &Module) -> Vec<Handle<Function>> {
    let mut found = Vec::new();
    for (handle, function) in module.functions.iter() {
        if function
            .expressions
            .iter()
            .any(|(_, expression)| takes_screen_derivative(expression))
            || calls_any(&function.body, &found)
        {
            found.push(handle);
        }
    }
    found
}

fn derivatives_in_loops(
    function: &Function,
    block: &Block,
    in_loop: bool,
    derivative_functions: &[Handle<Function>],
    module: &Module,
    found: &mut Vec<String>,
) {
    let name = function.name.as_deref().unwrap_or("?");
    for statement in block.iter() {
        match statement {
            Statement::Emit(range) if in_loop => {
                if range
                    .clone()
                    .any(|handle| takes_screen_derivative(&function.expressions[handle]))
                {
                    found.push(format!("{name} takes a screen derivative in a loop"));
                }
            }
            Statement::Call {
                function: callee, ..
            } if in_loop && derivative_functions.contains(callee) => {
                found.push(format!(
                    "{name} calls {} in a loop",
                    module.functions[*callee].name.as_deref().unwrap_or("?")
                ));
            }
            Statement::Block(inner) => derivatives_in_loops(
                function,
                inner,
                in_loop,
                derivative_functions,
                module,
                found,
            ),
            Statement::If { accept, reject, .. } => {
                for inner in [accept, reject] {
                    derivatives_in_loops(
                        function,
                        inner,
                        in_loop,
                        derivative_functions,
                        module,
                        found,
                    );
                }
            }
            Statement::Switch { cases, .. } => {
                for case in cases {
                    derivatives_in_loops(
                        function,
                        &case.body,
                        in_loop,
                        derivative_functions,
                        module,
                        found,
                    );
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                for inner in [body, continuing] {
                    derivatives_in_loops(
                        function,
                        inner,
                        true,
                        derivative_functions,
                        module,
                        found,
                    );
                }
            }
            _ => {}
        }
    }
}

#[test]
fn no_shader_takes_a_screen_derivative_inside_a_loop() {
    let shaders = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders");
    let mut found = Vec::new();
    let mut checked = 0;
    for entry in std::fs::read_dir(shaders).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "wgsl") {
            continue;
        }
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let module = wgsl::parse_str(&std::fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|error| panic!("{file}: {error}"));
        let derivative_functions = derivative_functions(&module);
        let mut in_file = Vec::new();
        for function in module
            .functions
            .iter()
            .map(|(_, function)| function)
            .chain(module.entry_points.iter().map(|entry| &entry.function))
        {
            derivatives_in_loops(
                function,
                &function.body,
                false,
                &derivative_functions,
                &module,
                &mut in_file,
            );
        }
        found.extend(in_file.into_iter().map(|line| format!("{file}: {line}")));
        checked += 1;
    }
    assert!(checked > 0);
    assert_eq!(
        found,
        ["cursor.wgsl: fs_main calls sample_cursor in a loop"]
    );
}
