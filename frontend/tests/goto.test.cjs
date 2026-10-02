const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const path = require("node:path");
const { test } = require("node:test");
const vm = require("node:vm");

const script = readFileSync(path.join(__dirname, "../assets/static/static/goto.js"), "utf8");

function submit(action, server, target) {
    let listener;
    let prevented = false;
    const form = {
        elements: { action: { value: action }, server: { value: server }, target: { value: target } },
        addEventListener(event, handler) {
            assert.equal(event, "submit");
            listener = handler;
        },
    };
    const context = { document: { getElementById: () => form }, window: { location: {} } };
    vm.runInNewContext(script, context);
    listener({ preventDefault() { prevented = true; } });
    assert.ok(prevented);
    return context.window.location.href;
}

test("summary ignores the target; whois omits the server; traceroute keeps it", () => {
    assert.equal(submit("summary", "edge+core", "show protocols"), "/summary/edge+core/");
    assert.equal(submit("whois", "edge", "AS64500"), "/whois/AS64500");
    assert.equal(submit("traceroute", "edge", "2001:db8::1"), "/traceroute/edge/2001%3Adb8%3A%3A1");
});

test("targets and legacy aliases survive URL encoding exactly once", () => {
    const target = "2001:db8::/32 + #?&% 上海";
    const result = submit("route", "上海+core", target);
    const parts = result.split("/");
    assert.equal(parts.length, 4);
    assert.equal(decodeURIComponent(parts[2]), "上海+core");
    assert.equal(decodeURIComponent(parts[3]), target);
    assert.equal(submit("route", "edge", ""), "/route/edge/");
    assert.equal(submit("route", "edge", "1.1.1.1"), "/route/edge/1.1.1.1");
});

test("map and primary extension actions stay selected on submission", () => {
    for (const action of ["route_bgpmap", "route_where_bgpmap", "route_from_protocol_primary", "route_from_origin_primary"]) {
        assert.equal(submit(action, "edge", "64500"), `/${action}/edge/64500`);
    }
    assert.equal(submit("not-a-query", "edge", "anything"), undefined);
});
