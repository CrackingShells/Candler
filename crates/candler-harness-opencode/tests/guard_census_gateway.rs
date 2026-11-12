#![cfg(test)]
//! Guards of the gateway census on the `OpenCode` side: the driver names the stream, and the
//! reader of the stream gives the tool inventory of the first request that carries tools, from
//! names alone.

use std::error::Error;

use candler_harness::driver::HarnessDriver as _;
use candler_harness::reader::FrameIn;
use candler_harness::spec::CensusSource;
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::raw::{FrameData, RawFrame, RawRef, StreamName};
use serde_json::{Value, json};

/// An inventory: the tool names and the source they came from.
type Inventory = (Vec<String>, CensusSource);

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

/// The tool names of an `OpenCode` model request, as `opencode_facts` recorded them.
const TOOLS: [&str; 12] = [
    "edit",
    "glob",
    "grep",
    "question",
    "read",
    "shell",
    "skill",
    "subagent",
    "webfetch",
    "websearch",
    "write",
    "execute",
];

/// What the driver's census reader observes after `lines`: the tools of the inventory and its
/// source, `None` for no inventory.
fn inventory(lines: &[String]) -> Res<(Option<Inventory>, String)> {
    let driver = OpenCodeDriver::new()?;
    let name = driver
        .census_stream()
        .ok_or("the driver names no census stream")?;
    let mut reader = driver
        .offline_reader(&name)
        .ok_or("the census stream has no reader")?;
    for (index, line) in lines.iter().enumerate() {
        let frame = RawFrame::new(None, FrameData::from_bytes(line.as_bytes()));
        let at = RawRef::new(name.clone(), u64::try_from(index)?);
        reader.feed(FrameIn::new(at, None, &frame))?;
    }
    let observed = reader.observed();
    let shown = format!("{observed:?}");
    let init = observed.init.map(|init| {
        let tools = init
            .tools
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned) // BORROW: .to_owned() makes the owned name
                    .collect()
            })
            .unwrap_or_default();
        (tools, init.source)
    });
    Ok((init, shown))
}

fn line(seq: u64, tools: &[&str]) -> String {
    json!({"seq": seq, "tools": tools}).to_string()
}

fn names(tools: &[&str]) -> Vec<String> {
    tools.iter().map(|tool| (*tool).to_owned()).collect() // BORROW: .to_owned() makes the owned names
}

/// Guards: amendment A1 — the inventory of a harness without an init event is the tool names of
/// the first model request that carries any, labelled as observed by the gateway; a title request
/// with no tools, listed first, is skipped; the first is the lowest sequence number, whatever
/// order the lines come in; a run none of whose requests carry tools has no inventory, never an
/// empty one; the driver declares the stream and the census capability.
/// Oracle: the A1 decision's conditions 2 to 4, and the twelve tool names of `opencode_facts`
/// (census section); the lines are written by hand.
/// Bite: the inventory taken from the first request whatever it carries (the title request),
/// from the last request, or recorded as an empty set when no request carries tools.
#[test]
fn census_gateway_inventory_is_the_first_request_with_tools() -> Res<()> {
    let rows: Vec<(&str, Vec<String>, Option<Inventory>)> = vec![
        (
            "a title request with no tools, then a request with tools",
            vec![line(1, &[]), line(2, &TOOLS), line(3, &TOOLS)],
            Some((names(&TOOLS), CensusSource::Gateway)),
        ),
        (
            "the first request with tools is the lowest sequence number, not the first line",
            vec![
                line(3, &["late"]),
                line(1, &[]),
                line(2, &["early", "tool"]),
            ],
            Some((names(&["early", "tool"]), CensusSource::Gateway)),
        ),
        (
            "a later request that lists other tools does not replace the first",
            vec![line(1, &["a"]), line(2, &["b"])],
            Some((names(&["a"]), CensusSource::Gateway)),
        ),
        (
            "no request carries tools",
            vec![line(1, &[]), line(2, &[])],
            None,
        ),
        ("no request at all", Vec::new(), None),
    ];
    for (case, lines, want) in rows {
        let (init, _) = inventory(&lines)?;
        assert_eq!(init, want, "{case}");
    }
    let driver = OpenCodeDriver::new()?;
    assert!(driver.capabilities().census);
    assert_eq!(
        driver.census_stream(),
        Some("gateway_census".parse::<StreamName>()?)
    );
    Ok(())
}

/// Guards: amendment A1 — names only: whatever else a census line carries (a description, a
/// schema, message text, a credential) is not in what the reader keeps, and a line that is not a
/// sequence number and a list of names is refused whole.
/// Oracle: the canary strings written into the lines here; the reader's whole observed view,
/// printed, holds none of them.
/// Bite: the reader keeping the line's other fields, or taking an object entry of `tools` as a
/// name.
#[test]
fn census_gateway_names_only_keeps_nothing_but_the_names() -> Res<()> {
    let rich = json!({
        "seq": 1,
        "tools": ["shell"],
        "description": "DESCRIPTION-CANARY",
        "input_schema": {"properties": {"x": {"description": "SCHEMA-CANARY"}}},
        "messages": [{"content": "MESSAGE-CANARY"}],
        "token": "TOKEN-CANARY",
    })
    .to_string();
    let (init, shown) = inventory(&[rich])?;
    assert_eq!(init, Some((names(&["shell"]), CensusSource::Gateway)));
    for canary in ["DESCRIPTION", "SCHEMA", "MESSAGE", "TOKEN"] {
        assert!(
            !shown.contains(canary),
            "{canary} reached the view: {shown}"
        );
    }
    let driver = OpenCodeDriver::new()?;
    let name = driver.census_stream().ok_or("no stream")?;
    for bad in [
        r#"{"seq":1,"tools":[{"name":"shell","description":"D"}]}"#,
        r#"{"seq":"1","tools":["shell"]}"#,
        r#"{"tools":["shell"]}"#,
        "not json",
    ] {
        let mut reader = driver.offline_reader(&name).ok_or("no reader")?;
        let frame = RawFrame::new(None, FrameData::from_bytes(bad.as_bytes()));
        let fed = reader.feed(FrameIn::new(RawRef::new(name.clone(), 0), None, &frame));
        assert!(fed.is_err(), "{bad} was read");
    }
    Ok(())
}
