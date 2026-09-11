mod common;

use common::{assert_eq_normalized, render_pipeline, render_pipeline_with_filename};

fn apply(input: &str) -> String {
    render_pipeline(input)
}

#[test]
fn converts_reflect_apply_for_standalone_functions() {
    let output = apply(
        r#"
Reflect.apply(fn, undefined, args);
Reflect.apply(fn, void 0, args);
Reflect.apply(fn, null, args);
Reflect.apply(fn, null, [first, second]);
Reflect.apply(fn, undefined, [{ ...options }]);
Reflect.apply(eval, undefined, [source]);
Reflect.apply(eval, undefined, args);
Reflect.apply(fn, undefined, ([first, second]));
Reflect.apply(fn, undefined, []);
Reflect.apply(fn, undefined, [first, , third]);
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
fn(...args);
fn(...args);
fn(...args);
fn(first, second);
fn({ ...options });
Reflect.apply(eval, undefined, [source]);
Reflect.apply(eval, undefined, args);
fn(first, second);
fn();
fn(first, void 0, third);
"#,
    );
}

#[test]
fn converts_reflect_apply_for_same_receiver_methods() {
    let output = apply(
        r#"
const object = {};
Reflect.apply(object.method, object, args);
Reflect.apply(object[key], object, args);
Reflect.apply(object.method, object, [first, , third]);
Reflect.apply(object.method, object, [first, ...rest]);
Reflect.apply(this.method, this, args);
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
const object = {};
object.method(...args);
object[key](...args);
object.method(first, void 0, third);
object.method(first, ...rest);
this.method(...args);
"#,
    );
}

#[test]
fn preserves_reflect_apply_when_receiver_semantics_are_not_proven() {
    let input = r#"
const stable = {};
let object = first;
Reflect.apply(stable.method, stable, args);
Reflect.apply(fn, undefined, args);
Reflect.apply(fn, object, args);
Reflect.apply(object.method, other, args);
Reflect.apply(object.method, null, args);
Reflect.apply(object.nested.method, object.nested, args);
Reflect.apply(getObject().method, getObject(), args);
Reflect.apply(getObject()[key()], getObject(), args);
let changing = first;
function changingKey() { changing = second; return "method"; }
Reflect.apply(changing[changingKey()], changing, args);
function wrapper(Reflect, args) { return Reflect.apply(undefined, args); }
function undefinedWrapper(undefined) { return Reflect.apply(fn, undefined, args); }
object = second;
"#;

    assert_eq_normalized(
        &apply(input),
        r#"
const stable = {};
let object = first;
stable.method(...args);
fn(...args);
Reflect.apply(fn, object, args);
Reflect.apply(object.method, other, args);
Reflect.apply(object.method, null, args);
Reflect.apply(object.nested.method, object.nested, args);
Reflect.apply(getObject().method, getObject(), args);
Reflect.apply(getObject()[key()], getObject(), args);
let changing = first;
function changingKey() { changing = second; return "method"; }
Reflect.apply(changing[changingKey()], changing, args);
function wrapper(Reflect, args) { return Reflect(...args); }
function undefinedWrapper(undefined) { return Reflect.apply(fn, undefined, args); }
object = second;
"#,
    );
}

#[test]
fn preserves_spread_arguments() {
    assert_eq_normalized(
        &apply(
            "Reflect.apply(...target, undefined, args);\nReflect.apply(fn, ...thisArg, args);\nReflect.apply(fn, undefined, ...args);\nReflect.apply(fn, undefined, [first, ...rest]);\nReflect.apply(fn, undefined, ([first, ...rest]));\nReflect.apply(fn, undefined, args);",
        ),
        "Reflect.apply(...target, undefined, args);\nReflect.apply(fn, ...thisArg, args);\nReflect.apply(fn, undefined, ...args);\nfn(first, ...rest);\nfn(first, ...rest);\nfn(...args);",
    );
}

#[test]
fn preserves_shadowed_reflect() {
    assert_eq_normalized(
        &apply(
            "function wrapper(Reflect) { return Reflect.apply(fn, undefined, args); }\n{ const Reflect = custom; Reflect.apply(fn, undefined, args); Reflect.apply(fn, undefined, args); }\nwith ({ Reflect: { apply() { sideEffect(); } } }) { Reflect.apply(fn, undefined, args); }\nReflect.apply(fn, undefined, args);",
        ),
        "function wrapper(Reflect) { return Reflect.apply(fn, undefined, args); }\n{ const Reflect = custom; Reflect.apply(fn, undefined, args); Reflect.apply(fn, undefined, args); }\nwith ({ Reflect: { apply() { sideEffect(); } } }) { Reflect.apply(fn, undefined, args); }\nfn(...args);",
    );
}

#[test]
fn preserves_eval_targets_and_parameter_eval_scopes() {
    assert_eq_normalized(
        &apply(
            r#"
function wrapper(eval) {
    return Reflect.apply(eval, undefined, args);
}
function run(value = eval("var Reflect = custom")) {
    return Reflect.apply(fn, undefined, args);
}
Reflect.apply(fn, undefined, args);
"#,
        ),
        r#"
function wrapper(eval) {
    return Reflect.apply(eval, undefined, args);
}
function run(value = eval("var Reflect = custom")) {
    return Reflect.apply(fn, undefined, args);
}
fn(...args);
"#,
    );
}

#[test]
fn preserves_reflect_only_inside_direct_eval_scopes() {
    assert_eq_normalized(
        &apply(
            r#"
function dynamic(fn, args) {
    eval("var Reflect = custom");
    return Reflect.apply(fn, undefined, args);
}
class Holder {
    value = eval("");
}
function unused() { eval(""); }
Reflect.apply(fn, undefined, args);
"#,
        ),
        r#"
function dynamic(fn, args) {
    eval("var Reflect = custom");
    return Reflect.apply(fn, undefined, args);
}
class Holder {
    value = eval("");
}
function unused() { eval(""); }
fn(...args);
"#,
    );
}

#[test]
fn covers_arrow_and_transparent_typescript_eval_scopes() {
    let output = render_pipeline_with_filename(
        r#"
const arrow = (args) => {
    eval("var Reflect = custom");
    return Reflect.apply(fn, undefined, args);
};
const parameterArrow = (value = eval("var Reflect = custom")) => Reflect.apply(fn, undefined, args);
Reflect.apply(fn, undefined, ([first, second] as unknown[]));
Reflect.apply(fn, undefined, (<unknown[]>[first, second]));
Reflect.apply(fn, undefined, args);
"#,
        "fixture.ts",
    );

    assert_eq_normalized(
        &output,
        r#"
const arrow = (args) => {
    eval("var Reflect = custom");
    return Reflect.apply(fn, undefined, args);
};
const parameterArrow = (value = eval("var Reflect = custom")) => Reflect.apply(fn, undefined, args);
fn(first, second);
fn(first, second);
fn(...args);
"#,
    );
}

#[test]
fn distinguishes_namespace_and_live_import_receivers() {
    let output = apply(
        r#"
import * as namespace from "./dependency";
import { receiver } from "./dependency";
import defaultReceiver from "./dependency";
Reflect.apply(namespace.method, namespace, args);
Reflect.apply(receiver.method, receiver, args);
Reflect.apply(defaultReceiver.method, defaultReceiver, args);
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
import * as namespace from "./dependency";
import defaultReceiver, { receiver } from "./dependency";
namespace.method(...args);
Reflect.apply(receiver.method, receiver, args);
Reflect.apply(defaultReceiver.method, defaultReceiver, args);
"#,
    );
}
