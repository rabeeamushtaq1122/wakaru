mod common;

use common::{assert_eq_normalized, render};

fn apply(input: &str) -> String {
    render(&format!("{TS_HELPERS}\n{input}"))
}

const TS_HELPERS: &str = r#"
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { step(generator.next(value)); }
        function rejected(value) { step(generator["throw"](value)); }
        function step(result) { result.done ? resolve(result.value) : Promise.resolve(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __generator = (this && this.__generator) || function (thisArg, body) {
    var state = { label: 0, sent: function() { return state[1]; }, trys: [], ops: [] };
    return body.call(thisArg, state);
};
"#;

#[test]
fn recovers_canonical_awaiter_frames() {
    let output = apply(
        r#"
function load() {
    return __awaiter(this, void 0, void 0, function* () {
        yield fetch_value();
    });
}
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
async function load() {
    await fetch_value();
}
"#,
    );
}

#[test]
fn preserves_noncanonical_awaiter_frames() {
    let output = render(
        r#"
function load() {
    return __awaiter(this, probe(), CustomPromise, function* (value) {
        yield value;
    });
}
"#,
    );

    assert!(output.contains("__awaiter"));
    assert!(output.contains("probe()"));
    assert!(output.contains("CustomPromise"));
}

#[test]
fn preserves_extra_and_spread_arguments() {
    let output = apply(
        r#"
function extra() {
    return __awaiter(this, void 0, void 0, function* () {}, extra_value);
}
function spread() {
    return __awaiter(this, ...frame);
}
"#,
    );

    assert!(output.contains("__awaiter"));
    assert!(output.contains("extra_value"));
    assert!(output.contains("...frame"));
}

#[test]
fn preserves_real_arguments_and_parameterized_generators() {
    let output = apply(
        r#"
function with_arguments() {
    return __awaiter(this, arguments, void 0, function* () {
        yield work();
    });
}
function with_parameter(value) {
    return __awaiter(this, arguments, void 0, function* (item) {
        yield item;
    });
}
"#,
    );

    assert!(output.contains("__awaiter"));
    assert!(output.contains("arguments"));
}

#[test]
fn preserves_shadowed_or_unresolved_frame_bindings() {
    let output = apply(
        r#"
function shadowed(undefined) {
    return __awaiter(undefined, void 0, void 0, function* () {
        yield this.value;
    });
}
function unresolved() {
    return __awaiter(missing, void 0, void 0, function* () {});
}
"#,
    );

    assert!(output.contains("__awaiter"));
    assert!(output.contains("missing"));
}

#[test]
fn accepts_global_promise_and_literal_void_frames() {
    let output = apply(
        r#"
function load() {
    return __awaiter(this, void 0, Promise, function* () {
        yield work();
    });
}
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
async function load() {
    await work();
}
"#,
    );
}

#[test]
fn accepts_all_canonical_receiver_and_placeholder_forms() {
    let output = apply(
        r#"
function void_receiver() {
    return __awaiter(void 0, undefined, void 0, function* () {});
}
function global_receiver() {
    return __awaiter(undefined, undefined, undefined, function* () {});
}
function alias_receiver() {
    var self = this;
    return __awaiter(self, undefined, Promise, function* () {
        yield this.value;
    });
}
"#,
    );

    assert_eq_normalized(
        &output,
        r#"
async function void_receiver() {}
async function global_receiver() {}
async function alias_receiver() {
    const self = this;
    await self.value;
}
"#,
    );
}

#[test]
fn preserves_noncanonical_slots_and_standalone_iifes() {
    let output = apply(
        r#"
function side_effecting_receiver() {
    return __awaiter(void probe(), void 0, void 0, function* () {
        yield work();
    });
}
function side_effecting_arguments() {
    return __awaiter(this, probe(), void 0, function* () {
        yield work();
    });
}
function side_effecting_promise() {
    return __awaiter(this, void 0, void probe(), function* () {
        yield work();
    });
}
function supplied_arguments() {
    return __awaiter(this, supplied, void 0, function* (item) {
        yield item;
    });
}
"#,
    );

    assert!(output.contains("void probe()"));
    assert!(output.contains("probe()"));
    assert!(output.contains("supplied"));
    assert!(output.contains("__awaiter"));

    let standalone = apply(
        r#"
__awaiter(this, void 0, void 0, function* () {
    yield work();
});
"#,
    );
    assert!(!standalone.contains("__awaiter("));
    assert!(standalone.contains("async"));
}

#[test]
fn preserves_shadowed_global_slots() {
    let output = apply(
        r#"
function shadowed_promise(Promise) {
    return __awaiter(this, void 0, Promise, function* () {
        yield work();
    });
}
function shadowed_arguments(undefined) {
    return __awaiter(this, undefined, void 0, function* () {
        yield work();
    });
}
"#,
    );

    assert!(output.contains("shadowed_promise"));
    assert!(output.contains("shadowed_arguments"));
    assert!(output.matches("__awaiter").count() >= 2);
}
