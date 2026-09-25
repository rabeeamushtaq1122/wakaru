mod common;

use common::{assert_eq_normalized, render_with_level};
use wakaru_core::RewriteLevel;

fn apply(source: &str, level: RewriteLevel) -> String {
    render_with_level(source, level)
}

#[test]
fn recovers_the_canonical_global_call() {
    let actual = apply(
        "const a = Object.prototype.hasOwnProperty.call(value, key); const b = wrap(Object.prototype.hasOwnProperty.call(other, name));",
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        "const a = Object.hasOwn(value, key); const b = wrap(Object.hasOwn(other, name));",
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn preserves_shadowed_and_dynamic_forms() {
    let actual = apply(
        r#"
function local(Object) { return Object.prototype.hasOwnProperty.call(value, key); }
const computed = Object.prototype[method].call(value, key);
const computedBase = Object[prototypeName].hasOwnProperty.call(value, key);
const wrongMethod = Object.prototype.propertyIsEnumerable.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
function local(Object) { return Object.prototype.hasOwnProperty.call(value, key); }
const computed = Object.prototype[method].call(value, key);
const computedBase = Object[prototypeName].hasOwnProperty.call(value, key);
const wrongMethod = Object.prototype.propertyIsEnumerable.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn preserves_invalid_argument_shapes() {
    let actual = apply(
        r#"
const missing = Object.prototype.hasOwnProperty.call(value);
const extra = Object.prototype.hasOwnProperty.call(value, key, extra);
const spread = Object.prototype.hasOwnProperty.call(...args);
const optional = Object?.prototype.hasOwnProperty.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
const missing = Object.prototype.hasOwnProperty.call(value);
const extra = Object.prototype.hasOwnProperty.call(value, key, extra);
const spread = Object.prototype.hasOwnProperty.call(...args);
const optional = Object?.prototype.hasOwnProperty.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn any_static_mutation_blocks_earlier_and_later_calls() {
    let actual = apply(
        r#"
const O = Object;
const P = Object.prototype;
const before = Object.prototype.hasOwnProperty.call(value, key);
O.prototype.hasOwnProperty = replacement;
const after = Object.prototype.hasOwnProperty.call(value, key);
delete P.hasOwnProperty;
const later = Object.prototype.hasOwnProperty.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
const before = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype.hasOwnProperty = replacement;
const after = Object.prototype.hasOwnProperty.call(value, key);
delete Object.prototype.hasOwnProperty;
const later = Object.prototype.hasOwnProperty.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn assignment_update_and_delete_are_all_barriers() {
    let actual = apply(
        r#"
Object = otherObject;
const a = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype = otherPrototype;
const prototypeReplaced = Object.prototype.hasOwnProperty.call(value, key);
delete Object.prototype.hasOwnProperty;
const b = Object.prototype.hasOwnProperty.call(value, key);
delete Object.prototype;
const prototypeDeleted = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype.hasOwnProperty++;
const c = Object.prototype.hasOwnProperty.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
Object = otherObject;
const a = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype = otherPrototype;
const prototypeReplaced = Object.prototype.hasOwnProperty.call(value, key);
delete Object.prototype.hasOwnProperty;
const b = Object.prototype.hasOwnProperty.call(value, key);
delete Object.prototype;
const prototypeDeleted = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype.hasOwnProperty++;
const c = Object.prototype.hasOwnProperty.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn aliases_to_object_and_prototype_are_mutation_barriers() {
    let actual = apply(
        r#"
const before = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype.hasOwnProperty = replacement;
const after = Object.prototype.hasOwnProperty.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
const before = Object.prototype.hasOwnProperty.call(value, key);
Object.prototype.hasOwnProperty = replacement;
const after = Object.prototype.hasOwnProperty.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn dynamic_scope_blocks_recovery_conservatively() {
    let actual = apply(
        r#"
with (scope) { consume(Object.prototype.hasOwnProperty.call(value, key)); }
const afterWith = Object.prototype.hasOwnProperty.call(value, key);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
with (scope) { consume(Object.prototype.hasOwnProperty.call(value, key)); }
const afterWith = Object.prototype.hasOwnProperty.call(value, key);
"#,
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn direct_eval_blocks_recovery_conservatively() {
    let actual = apply(
        "eval(code); const value = Object.prototype.hasOwnProperty.call(object, key);",
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        "eval(code); const value = Object.prototype.hasOwnProperty.call(object, key);",
    );
    assert_eq_normalized(
        &apply(
            "const safe = Object.prototype.hasOwnProperty.call(value, key);",
            RewriteLevel::Aggressive,
        ),
        "const safe = Object.hasOwn(value, key);",
    );
}

#[test]
fn preserves_existing_indirect_call_rewrites() {
    let actual = apply(
        "const direct = (0, fn)(value); const wrapped = Object(fn)(value);",
        RewriteLevel::Standard,
    );
    assert_eq_normalized(
        &actual,
        "const direct = fn(value); const wrapped = fn(value);",
    );
}

#[test]
fn recovery_is_aggressive_only() {
    let input = "const value = Object.prototype.hasOwnProperty.call(object, key);";
    assert_eq_normalized(&apply(input, RewriteLevel::Minimal), input);
    assert_eq_normalized(&apply(input, RewriteLevel::Standard), input);
    assert_eq_normalized(
        &apply(input, RewriteLevel::Aggressive),
        "const value = Object.hasOwn(object, key);",
    );
}

#[test]
fn a_shadowed_nested_scope_does_not_block_a_safe_sibling() {
    let actual = apply(
        r#"
function local(Object) { return Object.prototype.hasOwnProperty.call(value, key); }
const safe = Object.prototype.hasOwnProperty.call(other, name);
"#,
        RewriteLevel::Aggressive,
    );
    assert_eq_normalized(
        &actual,
        r#"
function local(Object) { return Object.prototype.hasOwnProperty.call(value, key); }
const safe = Object.hasOwn(other, name);
"#,
    );
}
