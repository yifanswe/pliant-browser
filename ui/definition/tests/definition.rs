use pliant_ui_definition::{
    parse_definition, resolve_address_open, AddressOpenResolution, AddressOpenTarget, Command,
    DefinitionState, ErrorKind, Node, StateErrorKind, BUILT_IN_SAFE_DEFINITION,
    MAX_DEFINITION_BYTES, MAX_NODE_COUNT, MAX_NODE_DEPTH, MAX_RUNTIME_ADDRESS_BYTES,
    MAX_STRING_BYTES, MAX_STRING_VALUE_BYTES, SCHEMA_VERSION,
};

#[test]
fn malformed_json_is_rejected_with_context() {
    let error = parse_definition(r#"{"schema_version": 1"#).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::MalformedJson);
    assert!(error.to_string().contains("definition JSON"));
}

#[test]
fn duplicate_object_keys_are_rejected_at_every_schema_level() {
    let definition = minimal_definition().replace(
        r#""address_open": "current_page""#,
        r#""address_open": "current_page", "address_open": "new_page""#,
    );
    let node =
        minimal_definition().replace(r#""id": "pages""#, r#""id": "pages", "id": "other-pages""#);
    let command = minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        r#"{
            "type": "button",
            "id": "new",
            "label": "New",
            "command": {"type": "back", "type": "new_page"}
          },
          {"type": "content_surface", "id": "content"}"#,
    );

    for (source, key) in [
        (definition, "address_open"),
        (node, "id"),
        (command, "type"),
    ] {
        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::MalformedJson);
        assert!(error.to_string().contains("duplicate"));
        assert!(error.to_string().contains(key));
    }
}

#[test]
fn trailing_json_input_remains_rejected() {
    let source = format!("{} true", minimal_definition());

    let error = parse_definition(&source).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::MalformedJson);
    assert!(error.to_string().contains("trailing"));
}

#[test]
fn composed_third_tree_parses_without_a_named_preset() {
    let source = r#"
    {
      "schema_version": 1,
      "address_open": "new_page",
      "root": {
        "type": "row",
        "id": "root",
        "gap": 8,
        "children": [
          {
            "type": "column",
            "id": "rail",
            "children": [
              {"type": "label", "id": "title", "text": "Research"},
              {"type": "page_list", "id": "pages"},
              {"type": "spacer", "id": "rail-space", "size": 12},
              {
                "type": "button",
                "id": "close",
                "label": "Close",
                "command": {"type": "close"}
              }
            ]
          },
          {
            "type": "column",
            "id": "main",
            "children": [
              {
                "type": "row",
                "id": "controls",
                "children": [
                  {
                    "type": "button",
                    "id": "back",
                    "label": "Back",
                    "command": {"type": "back"}
                  },
                  {
                    "type": "button",
                    "id": "forward",
                    "label": "Forward",
                    "command": {"type": "forward"}
                  },
                  {
                    "type": "address_field",
                    "id": "address",
                    "placeholder": "Open a site"
                  },
                  {
                    "type": "button",
                    "id": "docs",
                    "label": "Docs",
                    "command": {
                      "type": "navigate",
                      "url": "https://example.com/docs"
                    }
                  },
                  {
                    "type": "button",
                    "id": "new",
                    "label": "New",
                    "command": {"type": "new_page"}
                  }
                ]
              },
              {"type": "content_surface", "id": "content"}
            ]
          }
        ]
      }
    }
    "#;

    let definition = parse_definition(source).unwrap();

    assert_eq!(definition.schema_version(), SCHEMA_VERSION);
    assert_eq!(definition.address_open_target(), AddressOpenTarget::NewPage);
    let Node::Row(root) = definition.root() else {
        panic!("expected row root");
    };
    assert_eq!(root.id(), "root");
    assert_eq!(root.gap(), Some(8.0));
    assert_eq!(root.children().len(), 2);

    let Node::Column(rail) = &root.children()[0] else {
        panic!("expected column rail");
    };
    assert!(matches!(&rail.children()[0], Node::Label(label) if label.text() == "Research"));
    assert!(matches!(&rail.children()[1], Node::PageList(_)));
    assert!(matches!(&rail.children()[2], Node::Spacer(spacer) if spacer.size() == 12.0));
    assert!(
        matches!(&rail.children()[3], Node::Button(button) if button.command() == &Command::Close)
    );

    let Node::Column(main) = &root.children()[1] else {
        panic!("expected main column");
    };
    let Node::Row(controls) = &main.children()[0] else {
        panic!("expected nested controls row");
    };
    assert!(
        matches!(&controls.children()[0], Node::Button(button) if button.command() == &Command::Back)
    );
    assert!(
        matches!(&controls.children()[1], Node::Button(button) if button.command() == &Command::Forward)
    );
    assert!(
        matches!(&controls.children()[2], Node::AddressField(field) if field.placeholder() == Some("Open a site"))
    );
    assert!(matches!(
        &controls.children()[3],
        Node::Button(button)
            if button.command()
                == &Command::Navigate {
                    url: "https://example.com/docs".to_owned()
                }
    ));
    assert!(
        matches!(&controls.children()[4], Node::Button(button) if button.command() == &Command::NewPage)
    );
    assert!(matches!(&main.children()[1], Node::ContentSurface(_)));
}

#[test]
fn unsupported_schema_version_is_rejected() {
    let source = minimal_definition().replace(r#""schema_version": 1"#, r#""schema_version": 2"#);

    let error = parse_definition(&source).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::UnsupportedSchemaVersion);
    assert!(error.to_string().contains("schema_version"));
    assert!(error.to_string().contains('2'));
}

#[test]
fn unknown_fields_are_rejected_at_each_schema_level() {
    let top_level =
        minimal_definition().replace(r#""root": {"#, r#""unexpected": true, "root": {"#);
    let node = minimal_definition().replace(
        r#"{"type": "page_list", "id": "pages"}"#,
        r#"{"type": "page_list", "id": "pages", "unexpected": true}"#,
    );
    let command = minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        r#"{
            "type": "button",
            "id": "new",
            "label": "New",
            "command": {"type": "new_page", "unexpected": true}
          },
          {"type": "content_surface", "id": "content"}"#,
    );

    for (source, context) in [
        (top_level, "definition"),
        (node, "root.children[0]"),
        (command, "command"),
    ] {
        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnknownField);
        assert!(error.to_string().contains(context));
        assert!(error.to_string().contains("unexpected"));
    }
}

#[test]
fn unknown_or_privileged_commands_are_rejected() {
    for command in ["shutdown", "shell", "request_permission"] {
        let replacement = format!(
            r#"{{
              "type": "button",
              "id": "unsafe",
              "label": "Unsafe",
              "command": {{"type": "{command}"}}
            }},
            {{"type": "content_surface", "id": "content"}}"#
        );
        let source = minimal_definition().replace(
            r#"{"type": "content_surface", "id": "content"}"#,
            &replacement,
        );

        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnknownCommand);
        assert!(error.to_string().contains(command));
        assert!(error.to_string().contains("command"));
    }
}

#[test]
fn malformed_supported_commands_are_rejected_with_command_context() {
    for command in [r#"{"type": "navigate"}"#, r#""back""#, r#"{"type": 7}"#] {
        let replacement = format!(
            r#"{{
              "type": "button",
              "id": "invalid-command",
              "label": "Invalid",
              "command": {command}
            }},
            {{"type": "content_surface", "id": "content"}}"#
        );
        let source = minimal_definition().replace(
            r#"{"type": "content_surface", "id": "content"}"#,
            &replacement,
        );

        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidCommand);
        assert!(error.to_string().contains("root.children[2].command"));
    }
}

#[test]
fn duplicate_node_ids_are_rejected_with_both_locations() {
    let source = minimal_definition().replace(r#""id": "address""#, r#""id": "pages""#);

    let error = parse_definition(&source).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::DuplicateNodeId);
    assert!(error.to_string().contains("pages"));
    assert!(error.to_string().contains("root.children[0]"));
    assert!(error.to_string().contains("root.children[1]"));
}

#[test]
fn definitions_require_a_container_root_and_usable_host_controls() {
    let missing_content = minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        r#"{"type": "label", "id": "status", "text": "Ready"}"#,
    );
    let multiple_content = minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        r#"{"type": "content_surface", "id": "content-a"},
          {"type": "content_surface", "id": "content-b"}"#,
    );
    let missing_address = minimal_definition().replace(
        r#"{"type": "address_field", "id": "address"}"#,
        r#"{"type": "label", "id": "location", "text": "Location"}"#,
    );
    let missing_page_list = minimal_definition().replace(
        r#"{"type": "page_list", "id": "pages"}"#,
        r#"{"type": "label", "id": "pages-label", "text": "Pages"}"#,
    );
    let scalar_root = r#"
    {
      "schema_version": 1,
      "address_open": "current_page",
      "root": {"type": "content_surface", "id": "content"}
    }
    "#;

    for (source, context) in [
        (missing_content.as_str(), "exactly one content_surface"),
        (multiple_content.as_str(), "exactly one content_surface"),
        (missing_address.as_str(), "address_field"),
        (missing_page_list.as_str(), "page_list"),
        (scalar_root, "root must be a row or column"),
    ] {
        let error = parse_definition(source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidStructure);
        assert!(error.to_string().contains(context), "{error}");
    }
}

#[test]
fn invalid_ids_text_and_layout_sizes_are_rejected() {
    let empty_id = minimal_definition().replace(r#""id": "pages""#, r#""id": """#);
    let empty_label = minimal_definition().replace(
        r#"{"type": "page_list", "id": "pages"}"#,
        r#"{"type": "label", "id": "label", "text": ""}"#,
    );
    let negative_gap =
        minimal_definition().replace(r#""children": ["#, r#""gap": -1, "children": ["#);
    let excessive_gap =
        minimal_definition().replace(r#""children": ["#, r#""gap": 257, "children": ["#);
    let negative_spacer = definition_with_spacer_size("-1");
    let excessive_spacer = definition_with_spacer_size("4097");

    for (source, context) in [
        (empty_id, "id"),
        (empty_label, "text"),
        (negative_gap, "gap"),
        (excessive_gap, "gap"),
        (negative_spacer, "size"),
        (excessive_spacer, "size"),
    ] {
        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
        assert!(error.to_string().contains(context), "{error}");
    }

    let non_finite = definition_with_spacer_size("1e400");
    assert_eq!(
        parse_definition(&non_finite).unwrap_err().kind(),
        ErrorKind::MalformedJson
    );
}

#[test]
fn configured_navigation_rejects_unauthorized_url_schemes() {
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,hello",
        "chrome://settings",
    ] {
        let source = definition_with_navigation_url(url);

        let error = parse_definition(&source).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnauthorizedUrl);
        assert!(error.to_string().contains("navigate"));
        assert!(error.to_string().contains(url));
    }
}

#[test]
fn definition_byte_budget_is_enforced_before_parsing() {
    let mut source = minimal_definition();
    source.push_str(&" ".repeat(MAX_DEFINITION_BYTES + 1));

    let error = parse_definition(&source).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::InputTooLarge);
    assert!(error.to_string().contains("byte"));
    assert!(error
        .to_string()
        .contains(&MAX_DEFINITION_BYTES.to_string()));
}

#[test]
fn node_depth_budget_accepts_the_limit_and_rejects_the_next_level() {
    parse_definition(&definition_with_nested_columns(MAX_NODE_DEPTH - 2)).unwrap();

    let error = parse_definition(&definition_with_nested_columns(MAX_NODE_DEPTH - 1)).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::DepthLimitExceeded);
    assert!(error.to_string().contains("depth"));
    assert!(error.to_string().contains(&MAX_NODE_DEPTH.to_string()));
}

#[test]
fn node_count_budget_accepts_the_limit_and_rejects_one_more() {
    parse_definition(&definition_with_node_count(MAX_NODE_COUNT)).unwrap();

    let error = parse_definition(&definition_with_node_count(MAX_NODE_COUNT + 1)).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::NodeLimitExceeded);
    assert!(error.to_string().contains("node"));
    assert!(error.to_string().contains(&MAX_NODE_COUNT.to_string()));
}

#[test]
fn individual_and_aggregate_string_budgets_are_enforced() {
    let oversized_value = minimal_definition().replace(
        r#"{"type": "page_list", "id": "pages"}"#,
        &format!(
            r#"{{"type": "label", "id": "label", "text": "{}"}},
          {{"type": "page_list", "id": "pages"}}"#,
            "x".repeat(MAX_STRING_VALUE_BYTES + 1)
        ),
    );
    let error = parse_definition(&oversized_value).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::StringLimitExceeded);
    assert!(error.to_string().contains("text"));

    let repeated_labels = (0..9)
        .map(|index| {
            format!(
                r#"{{"type": "label", "id": "label-{index}", "text": "{}"}}"#,
                "x".repeat(MAX_STRING_VALUE_BYTES)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let aggregate = minimal_definition().replace(
        r#"{"type": "page_list", "id": "pages"}"#,
        &format!(
            r#"{repeated_labels},
          {{"type": "page_list", "id": "pages"}}"#
        ),
    );
    let error = parse_definition(&aggregate).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::StringLimitExceeded);
    assert!(error.to_string().contains("total string"));
    assert!(error.to_string().contains(&MAX_STRING_BYTES.to_string()));
}

#[test]
fn classic_and_workspace_presets_share_the_schema_but_not_the_tree() {
    let classic_source = include_str!("../../../presets/classic/definition.json");
    let workspace_source = include_str!("../../../presets/workspace/definition.json");

    let classic = parse_definition(classic_source).unwrap();
    let workspace = parse_definition(workspace_source).unwrap();

    assert_eq!(classic.schema_version(), SCHEMA_VERSION);
    assert_eq!(workspace.schema_version(), SCHEMA_VERSION);
    assert_eq!(
        classic.address_open_target(),
        AddressOpenTarget::CurrentPage
    );
    assert_eq!(workspace.address_open_target(), AddressOpenTarget::NewPage);
    assert!(matches!(classic.root(), Node::Column(_)));
    assert!(matches!(workspace.root(), Node::Row(_)));
    assert_ne!(classic.root(), workspace.root());
}

#[test]
fn address_open_policy_selects_current_or_new_page() {
    assert_eq!(
        resolve_address_open(
            AddressOpenTarget::CurrentPage,
            Some("page-7"),
            "https://example.com/current"
        )
        .unwrap(),
        AddressOpenResolution::NavigateCurrent {
            page_id: "page-7".to_owned(),
            url: "https://example.com/current".to_owned(),
        }
    );
    assert_eq!(
        resolve_address_open(
            AddressOpenTarget::NewPage,
            Some("page-7"),
            "https://example.com/new"
        )
        .unwrap(),
        AddressOpenResolution::OpenNewPage {
            url: "https://example.com/new".to_owned(),
        }
    );
    assert_eq!(
        resolve_address_open(
            AddressOpenTarget::CurrentPage,
            None,
            "https://example.com/first"
        )
        .unwrap(),
        AddressOpenResolution::OpenNewPage {
            url: "https://example.com/first".to_owned(),
        }
    );
}

#[test]
fn address_open_rejects_unauthorized_dynamic_urls() {
    for address in [
        "javascript:alert(1)",
        "file:///tmp/private",
        "data:text/html,hello",
        "example.com/no-scheme",
        " https://example.com",
    ] {
        let error = resolve_address_open(AddressOpenTarget::NewPage, None, address).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::UnauthorizedUrl);
        assert!(error.to_string().contains(address));
        assert!(error.to_string().contains("runtime address"));
    }
}

#[test]
fn runtime_address_byte_budget_accepts_the_limit_and_rejects_one_more() {
    let prefix = "https://example.com/";
    let exact = format!(
        "{prefix}{}",
        "a".repeat(MAX_RUNTIME_ADDRESS_BYTES - prefix.len())
    );
    let over = format!("{exact}a");

    assert_eq!(exact.len(), MAX_RUNTIME_ADDRESS_BYTES);
    resolve_address_open(AddressOpenTarget::NewPage, None, &exact).unwrap();

    let error = resolve_address_open(AddressOpenTarget::NewPage, None, &over).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InputTooLarge);
    assert!(error
        .to_string()
        .contains(&MAX_RUNTIME_ADDRESS_BYTES.to_string()));
}

#[test]
fn runtime_address_budget_counts_multibyte_utf8_bytes() {
    let address = format!(
        "https://example.com/{}",
        "\u{00e9}".repeat(MAX_RUNTIME_ADDRESS_BYTES / 2)
    );

    assert!(address.chars().count() < MAX_RUNTIME_ADDRESS_BYTES);
    assert!(address.len() > MAX_RUNTIME_ADDRESS_BYTES);
    let error = resolve_address_open(AddressOpenTarget::NewPage, None, &address).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::InputTooLarge);
}

#[test]
fn replacing_a_preview_makes_the_old_candidate_stale() {
    let initial = minimal_definition();
    let candidate_a = include_str!("../../../presets/classic/definition.json");
    let candidate_b = include_str!("../../../presets/workspace/definition.json");
    let mut state = DefinitionState::new(&initial).unwrap();

    let candidate_a_id = state.preview(candidate_a).unwrap();
    assert_eq!(state.active().source(), initial);
    let candidate_b_id = state.preview(candidate_b).unwrap();
    assert_ne!(candidate_a_id, candidate_b_id);
    assert_eq!(state.active().source(), initial);

    let error = state.apply(candidate_a_id).unwrap_err();
    assert_eq!(error.kind(), StateErrorKind::StaleCandidate);
    assert_eq!(state.pending_candidate_id(), Some(candidate_b_id));
    assert_eq!(state.active().source(), initial);

    state.apply(candidate_b_id).unwrap();
    assert_eq!(state.active().source(), candidate_b);
    assert_eq!(state.pending_candidate_id(), None);
}

#[test]
fn candidate_ids_cannot_be_applied_across_states() {
    let initial = minimal_definition();
    let candidate_a = include_str!("../../../presets/classic/definition.json");
    let candidate_b = include_str!("../../../presets/workspace/definition.json");
    let mut state_a = DefinitionState::new(&initial).unwrap();
    let mut state_b = DefinitionState::new(&initial).unwrap();
    let candidate_a_id = state_a.preview(candidate_a).unwrap();
    let candidate_b_id = state_b.preview(candidate_b).unwrap();

    assert_ne!(candidate_a_id, candidate_b_id);
    assert_eq!(
        state_b.apply(candidate_a_id).unwrap_err().kind(),
        StateErrorKind::StaleCandidate
    );
    assert_eq!(state_b.pending_candidate_id(), Some(candidate_b_id));
    assert_eq!(state_b.active().source(), initial);

    state_b.apply(candidate_b_id).unwrap();
    assert_eq!(state_b.active().source(), candidate_b);
}

#[test]
fn candidate_ids_cannot_be_rejected_across_states() {
    let initial = minimal_definition();
    let candidate_a = include_str!("../../../presets/classic/definition.json");
    let candidate_b = include_str!("../../../presets/workspace/definition.json");
    let mut state_a = DefinitionState::new(&initial).unwrap();
    let mut state_b = DefinitionState::new(&initial).unwrap();
    let candidate_a_id = state_a.preview(candidate_a).unwrap();
    let candidate_b_id = state_b.preview(candidate_b).unwrap();

    assert_eq!(
        state_b.reject(candidate_a_id).unwrap_err().kind(),
        StateErrorKind::StaleCandidate
    );
    assert_eq!(state_b.pending_candidate_id(), Some(candidate_b_id));
}

#[test]
fn cloned_states_remint_pending_candidate_ids() {
    let initial = minimal_definition();
    let candidate = include_str!("../../../presets/classic/definition.json");
    let mut state = DefinitionState::new(&initial).unwrap();
    let original_id = state.preview(candidate).unwrap();
    let mut cloned = state.clone();
    let cloned_id = cloned.pending_candidate_id().unwrap();

    assert_ne!(original_id, cloned_id);
    assert_eq!(
        state.apply(cloned_id).unwrap_err().kind(),
        StateErrorKind::StaleCandidate
    );
    assert_eq!(
        cloned.reject(original_id).unwrap_err().kind(),
        StateErrorKind::StaleCandidate
    );

    state.apply(original_id).unwrap();
    cloned.apply(cloned_id).unwrap();
    assert_eq!(state.active().source(), candidate);
    assert_eq!(cloned.active().source(), candidate);
}

#[test]
fn failed_preview_preserves_active_and_existing_candidate() {
    let initial = minimal_definition();
    let candidate = include_str!("../../../presets/classic/definition.json");
    let mut state = DefinitionState::new(&initial).unwrap();
    let candidate_id = state.preview(candidate).unwrap();

    let error = state.preview(r#"{"schema_version": 1"#).unwrap_err();

    assert_eq!(error.kind(), ErrorKind::MalformedJson);
    assert_eq!(state.active().source(), initial);
    let (pending_id, pending_snapshot) = state.pending().unwrap();
    assert_eq!(pending_id, candidate_id);
    assert_eq!(pending_snapshot.source(), candidate);
}

#[test]
fn apply_uses_the_exact_previewed_content_after_the_caller_changes_its_string() {
    let initial = minimal_definition();
    let mut candidate = include_str!("../../../presets/classic/definition.json").to_owned();
    let exact_preview = candidate.clone();
    let mut state = DefinitionState::new(&initial).unwrap();
    let candidate_id = state.preview(&candidate).unwrap();

    candidate.clear();
    candidate.push_str(include_str!("../../../presets/workspace/definition.json"));
    state.apply(candidate_id).unwrap();

    assert_eq!(state.active().source(), exact_preview);
    assert_eq!(
        state.active().definition().address_open_target(),
        AddressOpenTarget::CurrentPage
    );
}

#[test]
fn reject_preserves_active_and_reset_restores_the_builtin_safe_definition() {
    let initial = include_str!("../../../presets/workspace/definition.json");
    let candidate = include_str!("../../../presets/classic/definition.json");
    let mut state = DefinitionState::new(initial).unwrap();

    let rejected_id = state.preview(candidate).unwrap();
    state.reject(rejected_id).unwrap();
    assert_eq!(state.active().source(), initial);
    assert_eq!(state.pending_candidate_id(), None);
    assert_eq!(
        state.apply(rejected_id).unwrap_err().kind(),
        StateErrorKind::StaleCandidate
    );

    let applied_id = state.preview(candidate).unwrap();
    state.apply(applied_id).unwrap();
    assert_eq!(state.active().source(), candidate);
    state.preview(initial).unwrap();

    state.reset();

    assert_eq!(state.active().source(), BUILT_IN_SAFE_DEFINITION);
    assert_eq!(state.pending_candidate_id(), None);
    assert!(matches!(
        state.active().definition().root(),
        Node::Column(_)
    ));
}

fn definition_with_node_count(total: usize) -> String {
    assert!(total >= 4);
    let labels = (0..total - 4)
        .map(|index| format!(r#"{{"type": "label", "id": "label-{index}", "text": "x"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{
          "schema_version": 1,
          "address_open": "current_page",
          "root": {{
            "type": "column",
            "id": "root",
            "children": [
              {{"type": "page_list", "id": "pages"}},
              {{"type": "address_field", "id": "address"}},
              {{"type": "content_surface", "id": "content"}},
              {labels}
            ]
          }}
        }}"#
    )
}

fn definition_with_nested_columns(nested_containers: usize) -> String {
    let mut nested = r#"{"type": "content_surface", "id": "content"}"#.to_owned();
    for index in 0..nested_containers {
        nested = format!(r#"{{"type": "column", "id": "nested-{index}", "children": [{nested}]}}"#);
    }
    format!(
        r#"{{
          "schema_version": 1,
          "address_open": "current_page",
          "root": {{
            "type": "column",
            "id": "root",
            "children": [
              {{"type": "page_list", "id": "pages"}},
              {{"type": "address_field", "id": "address"}},
              {nested}
            ]
          }}
        }}"#
    )
}

fn definition_with_navigation_url(url: &str) -> String {
    minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        &format!(
            r#"{{
              "type": "button",
              "id": "go",
              "label": "Go",
              "command": {{"type": "navigate", "url": "{url}"}}
            }},
            {{"type": "content_surface", "id": "content"}}"#
        ),
    )
}

fn definition_with_spacer_size(size: &str) -> String {
    minimal_definition().replace(
        r#"{"type": "content_surface", "id": "content"}"#,
        &format!(
            r#"{{"type": "spacer", "id": "space", "size": {size}}},
          {{"type": "content_surface", "id": "content"}}"#
        ),
    )
}

fn minimal_definition() -> String {
    r#"
    {
      "schema_version": 1,
      "address_open": "current_page",
      "root": {
        "type": "column",
        "id": "root",
        "children": [
          {"type": "page_list", "id": "pages"},
          {"type": "address_field", "id": "address"},
          {"type": "content_surface", "id": "content"}
        ]
      }
    }
    "#
    .to_owned()
}
