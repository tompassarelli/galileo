# North slice transition reference

This corpus holds 43 recordings of North tests at North revision
`eaee1838d1968b7caba65195866e5d52fd30b2e1` (DECISIONS D-74, D-80):

- 8.13.0's 33 tests, named by PLAN boxes 8.13.1r to 8.13.20r.
  `restart_storage_fresh_process` is named by both 8.13.2r and 8.13.9r and is
  recorded once.
- 8.13.0c's 5 existing North tests outside those 33, each applying handlers
  the 33 never apply.
- 8.13.0d's `unreached_text_commands` and 8.13.0e's 4 `unreached_*` tests:
  drivers in an unlanded `src/galileo_unreached.rs` that call production
  `NorthState` methods for handlers no existing test applies.

The Clause compiler/runtime pin is `258e6956aa11dab2e724e89439e864e5e5dafdd6`
(MIT OR Apache-2.0). Only public Clause APIs were used; no Clause source is
copied here. Every recording's session records carry the same `north.clause`
digest, `fb347f7a89606c6983026ea2241a4d2866544beac2f126c9de7a982d1e4f1823`.

The recorder and the record format are 8.5's, unchanged; see
`tests/external/north-menu/README.md` for the meaning of each record kind
(sessions, ordered dispatcher occurrences and arguments, occurrence admissions,
ordered effects and the boundary mailbox, transaction begin/commit boundaries,
full canonical Clause projections in `projection_hex`, and continuation and
restore markers) and of the coverage file's handler dispatch and compiled clause
counts. Projections are sampled after the six appended maintenance handlers.
Raw entity identities are not an equivalence key: a Galileo replay must
establish the creation-order correspondence required by SPEC §10.4.

Each test was run separately in the North recorder lane, with
`NORTH_TRANSITION_RECORD` set to its JSONL path, as one exact test:

```sh
NORTH_TRANSITION_RECORD=/absolute/path/NAME.jsonl nix develop -c cargo test --offline --locked --bin north MODULE::NAME -- --exact --nocapture --test-threads=1
```

Cargo ran under the machine-capacity helper. The recorder appends JSONL and
accumulates its sibling `NAME.coverage.json` by session, so each output path was
removed before its run. Every test passed while recording.

North recorder commit: `2daaad3a60a62d8e8479818c2a7d7b02aa483e58`, published
with the same tree and a noreply author as
`293390b4651e4a1ed6e56aedcb5d631424824e72` on North's
`rescue/8.5-recorder-2daaad3`; its parent is the North revision above. 8.13.0
and 8.13.0c recorded with it, changing no North source. 8.13.0d's driver is
North commit `de9d3b09` and 8.13.0e's is `243d9a6d`, its child; both add only
`src/galileo_unreached.rs` and its `#[cfg(test)]` module line in `src/main.rs`.
All run from `~/code/north/worktrees/galileo-slice-recorder` on branch
`galileo-slice-recorder`, unmerged.

Recorded on 2026-10-12. Source names the PLAN box that recorded the test.
Admissions count occurrence admissions, Effects the per-transaction effect
records, Transactions the committed transactions, Handlers the distinct handler
names dispatched in that test, and Applied the distinct handler names the
applied check (below) names in it.

| Test | Source | Sessions | Transactions | Admissions | Effects | Projections | Handlers | Applied | Bytes |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `switching_running_conversations_keeps_drafts_and_interrupts_only_the_selected_turn` | 8.13.0 | 1 | 50 | 371 | 50 | 50 | 33 | 26 | 3,760,692 |
| `clause_owns_new_and_switched_conversation_identity` | 8.13.0 | 1 | 7 | 51 | 7 | 7 | 13 | 7 | 510,630 |
| `saved_workspace_handles_navigation_while_reconnection_is_pending` | 8.13.0 | 1 | 6 | 45 | 6 | 6 | 15 | 7 | 434,624 |
| `restart_storage_fresh_process` | 8.13.0 | 2 | 27 | 195 | 27 | 27 | 30 | 24 | 2,010,217 |
| `usage_display_distinguishes_missing_values_and_preserves_conversation_scope` | 8.13.0 | 1 | 10 | 85 | 10 | 10 | 17 | 13 | 862,157 |
| `reference_catalog_can_be_replaced_after_a_full_listing` | 8.13.0 | 1 | 3 | 535 | 3 | 3 | 10 | 4 | 1,594,934 |
| `escape_closes_references_and_typing_reopens_them` | 8.13.0 | 1 | 4 | 32 | 4 | 4 | 11 | 5 | 295,259 |
| `chat_batch_preserves_delta_order_and_final_replacement` | 8.13.0 | 1 | 5 | 38 | 5 | 5 | 11 | 6 | 375,578 |
| `transcript_controls_scroll_filter_changes_and_copy_without_changing_the_draft` | 8.13.0 | 1 | 28 | 196 | 28 | 28 | 12 | 7 | 2,450,187 |
| `approval_uses_only_offered_decisions_and_retains_exact_request_identity` | 8.13.0 | 1 | 14 | 107 | 14 | 14 | 19 | 15 | 1,055,270 |
| `multiple_questions_accept_choices_and_free_text_without_crossing_threads` | 8.13.0 | 1 | 13 | 101 | 13 | 13 | 19 | 15 | 1,048,454 |
| `tool_forms_use_checked_choices_validate_fields_and_keep_the_chat_draft` | 8.13.0 | 1 | 20 | 156 | 20 | 20 | 21 | 17 | 1,773,572 |
| `cancellation_is_explicit_and_unknown_delivery_never_resends` | 8.13.0 | 1 | 9 | 68 | 9 | 9 | 17 | 13 | 684,957 |
| `running_input_becomes_steering_and_failed_delivery_stays_unsent` | 8.13.0 | 1 | 12 | 89 | 12 | 12 | 22 | 12 | 827,138 |
| `queued_inputs_wait_for_settlement_and_keep_their_own_attachments` | 8.13.0 | 1 | 19 | 147 | 19 | 19 | 25 | 18 | 1,360,968 |
| `steering_waits_for_receipts_and_expiry_never_retargets_a_new_turn` | 8.13.0 | 1 | 17 | 128 | 17 | 17 | 22 | 14 | 1,189,572 |
| `conversation_turns_queues_and_drafts_are_independent_of_selection` | 8.13.0 | 1 | 22 | 165 | 22 | 22 | 26 | 18 | 1,575,173 |
| `clause_owns_submit_and_settlement` | 8.13.0 | 1 | 3 | 23 | 3 | 3 | 11 | 4 | 222,981 |
| `clause_owns_delegation_admission_and_settlement` | 8.13.0 | 1 | 5 | 37 | 5 | 5 | 12 | 5 | 355,017 |
| `clause_owns_delegation_failure_before_and_after_spawn` | 8.13.0 | 2 | 7 | 53 | 7 | 7 | 13 | 6 | 512,151 |
| `clause_owns_image_identity_draft_rollover_and_submission_membership` | 8.13.0 | 1 | 7 | 54 | 7 | 7 | 15 | 8 | 490,210 |
| `clause_rejects_unknown_switch_and_preserves_active_conversation_on_failure` | 8.13.0 | 1 | 7 | 50 | 7 | 7 | 13 | 7 | 502,479 |
| `slash_popup_exposes_the_basic_command_surface` | 8.13.0 | 1 | 1 | 7 | 1 | 1 | 7 | 1 | 90,833 |
| `checkpoint_failure_does_not_release_a_host_effect` | 8.13.0 | 2 | 5 | 37 | 5 | 5 | 13 | 5 | 417,851 |
| `spec_6_work_commands_route_without_a_coding_turn` | 8.13.0 | 1 | 25 | 175 | 25 | 25 | 10 | 4 | 1,675,473 |
| `history_commands_require_archive_confirmation_and_pause_archived_work` | 8.13.0 | 1 | 23 | 165 | 23 | 23 | 24 | 17 | 1,609,655 |
| `usage_compaction_waits_for_completion_without_requiring_an_answer` | 8.13.0 | 1 | 16 | 119 | 16 | 16 | 25 | 18 | 1,158,477 |
| `local_settings_effect_is_saved_with_its_completed_menu` | 8.13.0 | 2 | 6 | 51 | 6 | 6 | 20 | 11 | 502,422 |
| `clause_creates_orders_selects_and_edits_goals_with_revisions` | 8.13.0 | 1 | 24 | 168 | 24 | 24 | 14 | 8 | 1,657,764 |
| `clause_drives_the_tui_goal_create_inspect_edit_inspect_journey` | 8.13.0 | 1 | 19 | 139 | 19 | 19 | 16 | 11 | 1,386,452 |
| `inline_goal_commands_edit_and_clear_without_submitting_a_turn` | 8.13.0 | 1 | 19 | 139 | 19 | 19 | 11 | 6 | 1,409,593 |
| `goals_view_renders_its_empty_state` | 8.13.0 | 1 | 3 | 21 | 3 | 3 | 9 | 3 | 222,903 |
| `relation_decode_reuses_unchanged_history_and_observes_replacement` | 8.13.0 | 1 | 4 | 28 | 4 | 4 | 9 | 4 | 1,199,624 |
| `spec_6_closed_blocked_held_and_ready_items_show_their_status` | 8.13.0c | 1 | 5 | 44 | 5 | 5 | 15 | 8 | 389,501 |
| `tab_and_arrow_keys_navigate_the_two_product_views` | 8.13.0c | 1 | 7 | 49 | 7 | 7 | 12 | 6 | 484,801 |
| `steering_keys_send_the_active_turn_and_keep_rejected_images_editable` | 8.13.0c | 1 | 20 | 149 | 20 | 20 | 29 | 22 | 1,433,005 |
| `reference_menu_filters_renders_and_inserts_without_submitting` | 8.13.0c | 1 | 6 | 46 | 6 | 6 | 12 | 5 | 434,859 |
| `snapshot_replay_preserves_full_history_drafts_and_acceptance` | 8.13.0c | 2 | 11 | 88 | 11 | 11 | 22 | 15 | 2,345,704 |
| `unreached_text_commands` | 8.13.0d | 1 | 30 | 210 | 30 | 30 | 20 | 14 | 2,005,296 |
| `unreached_connection_loss_and_reconnect` | 8.13.0e | 1 | 6 | 49 | 6 | 6 | 17 | 12 | 466,152 |
| `unreached_failed_new_conversation` | 8.13.0e | 1 | 3 | 21 | 3 | 3 | 9 | 3 | 221,718 |
| `unreached_abandoned_turn` | 8.13.0e | 1 | 5 | 38 | 5 | 5 | 14 | 7 | 366,563 |
| `unreached_cleared_chat` | 8.13.0e | 1 | 5 | 36 | 5 | 5 | 12 | 7 | 370,913 |

The 43 recordings are 86 files (each JSONL with its coverage file) totalling
43,810,666 bytes (41.78 MiB), under the 64 MB bound.

## Handler coverage

A handler's oracle is its applications, not its dispatches (D-80): a dispatched
occurrence whose evaluation selects no rule changes nothing. The applied check
prints each handler name that a recording applies at least once, from the
occurrences whose evaluation has `rule_applied: true`. Session, transaction and
ordinal identify an occurrence only within one recording, so it runs per file:

```sh
for f in tests/external/north-slices/*.jsonl tests/external/north-menu/*.jsonl; do
  jq -rs '(map(select(.kind=="evaluation" and .rule_applied)|"\(.session)/\(.transaction)/\(.ordinal)")|map({(.):1})|add) as $a | .[]|select(.kind=="occurrence" and $a["\(.session)/\(.transaction)/\(.ordinal)"]==1)|.handler' "$f"
done | sort -u | wc -l
```

It prints 134. `north.clause` at the North revision declares 135 handler names
(`on NAME` lines, 150 clauses). Over the 43 recordings, 131 are dispatched and
130 apply; the menu corpus (`tests/external/north-menu/`, the same revision's
`north.clause`) applies four more: `clear-menu-rows`, `close-menu`,
`offer-history-row` and `open-history-menu`. Together they apply 134 of 135.
Dispatches and Applications are summed over every occurrence of the 43
recordings; Menu applications over the menu corpus.

| Handler | Dispatches | Applications | Menu applications |
| --- | ---: | ---: | ---: |
| `abandon-turn` | 1 | 1 | 0 |
| `accept-menu` | 1 | 1 | 1 |
| `answer-prompt-choice` | 4 | 4 | 0 |
| `answer-prompt-multiple` | 1 | 1 | 0 |
| `answer-prompt-text` | 4 | 4 | 0 |
| `append-chat` | 41 | 41 | 0 |
| `attach-image` | 13 | 13 | 0 |
| `begin-conversation-reconciliation` | 1 | 1 | 0 |
| `begin-reconnect` | 1 | 1 | 0 |
| `bind-input-client-id` | 5 | 5 | 0 |
| `cancel-prompt` | 1 | 1 | 0 |
| `child-spawned` | 2 | 2 | 0 |
| `clear-chat` | 1 | 1 | 0 |
| `clear-host-effect` | 39 | 39 | 1 |
| `clear-menu-rows` | 2 | 0 | 1 |
| `clear-references` | 5 | 2 | 0 |
| `clear-resolved-answers` | 538 | 6 | 0 |
| `clear-submitted-attachment` | 2 | 2 | 0 |
| `clear-turn` | 17 | 17 | 0 |
| `close-menu` | 0 | 0 | 1 |
| `close-turn-prompts` | 7 | 2 | 0 |
| `command-agents` | 1 | 1 | 0 |
| `command-archive` | 3 | 3 | 0 |
| `command-archived` | 1 | 1 | 0 |
| `command-compact` | 1 | 1 | 0 |
| `command-config` | 2 | 2 | 0 |
| `command-delegate` | 1 | 1 | 0 |
| `command-effort` | 1 | 1 | 0 |
| `command-fork` | 1 | 1 | 0 |
| `command-goal` | 13 | 13 | 0 |
| `command-goals` | 3 | 3 | 0 |
| `command-model` | 1 | 1 | 0 |
| `command-new` | 1 | 1 | 0 |
| `command-quit` | 1 | 1 | 0 |
| `command-reconnect` | 1 | 1 | 0 |
| `command-rename` | 1 | 1 | 0 |
| `command-resume` | 1 | 1 | 0 |
| `command-usage` | 1 | 1 | 0 |
| `command-work` | 8 | 8 | 0 |
| `compaction-completed` | 1 | 1 | 0 |
| `connection-lost` | 6 | 6 | 1 |
| `copy-queued-to-submitted` | 1 | 1 | 0 |
| `copy-submission-attachment` | 2 | 2 | 0 |
| `count-menu` | 3 | 3 | 12 |
| `count-usage` | 538 | 3 | 0 |
| `delegate` | 4 | 4 | 0 |
| `delegate-task` | 1 | 1 | 0 |
| `detach-image` | 3 | 3 | 0 |
| `disconnect-prompts` | 6 | 1 | 0 |
| `dismiss-references` | 2 | 2 | 0 |
| `ensure-context` | 38 | 23 | 0 |
| `expire-steering` | 17 | 1 | 0 |
| `fail-delegation-after-child` | 1 | 1 | 0 |
| `fail-delegation-before-child` | 1 | 1 | 0 |
| `fail-new-conversation` | 1 | 1 | 0 |
| `fail-switch-conversation` | 1 | 1 | 0 |
| `filter-menu` | 1 | 1 | 11 |
| `filter-transcript` | 538 | 121 | 0 |
| `finish-observed-turn` | 7 | 4 | 0 |
| `finish-reconnect` | 1 | 1 | 0 |
| `focus-prompts` | 538 | 47 | 0 |
| `forget-input` | 4 | 4 | 0 |
| `forget-input-attachment` | 1 | 1 | 0 |
| `goal-edit-input` | 3 | 3 | 0 |
| `goal-objective` | 4 | 4 | 0 |
| `goal-title` | 4 | 4 | 0 |
| `history-action-completed` | 1 | 1 | 0 |
| `initialize` | 44 | 44 | 2 |
| `input-receipt` | 6 | 5 | 0 |
| `inspect` | 0 | 0 | 0 |
| `move-menu` | 1 | 1 | 1 |
| `move-reference` | 3 | 3 | 0 |
| `navigate-prompt` | 4 | 4 | 0 |
| `observe-archived` | 1 | 1 | 0 |
| `observe-chat-item` | 10 | 10 | 0 |
| `observe-context-usage` | 2 | 2 | 0 |
| `observe-conversation` | 3 | 3 | 0 |
| `observe-input-acceptance` | 4 | 4 | 0 |
| `observe-prompt` | 7 | 6 | 0 |
| `observe-prompt-option` | 24 | 21 | 0 |
| `observe-prompt-question` | 14 | 13 | 0 |
| `observe-quota-window` | 4 | 4 | 0 |
| `observe-reset-credits` | 2 | 2 | 0 |
| `observe-restored` | 1 | 1 | 0 |
| `observe-settings` | 6 | 6 | 0 |
| `observe-stored-turn` | 2 | 2 | 0 |
| `observe-turn` | 9 | 9 | 0 |
| `offer-history-row` | 0 | 0 | 6 |
| `offer-reference` | 517 | 260 | 0 |
| `offer-settings-row` | 2 | 2 | 1 |
| `offer-work-row` | 4 | 4 | 0 |
| `open-history-menu` | 0 | 0 | 2 |
| `open-settings-menu` | 2 | 2 | 1 |
| `pause-disconnected-inputs` | 6 | 1 | 0 |
| `prepare-input-edit` | 1 | 1 | 0 |
| `prepare-prompt-response` | 8 | 4 | 0 |
| `prepare-queued-input` | 11 | 6 | 0 |
| `prepare-steering` | 7 | 4 | 0 |
| `present-prompt-questions` | 538 | 47 | 0 |
| `present-usage` | 538 | 7 | 0 |
| `prompt-answer-problem` | 3 | 3 | 0 |
| `prompt-response-written` | 2 | 1 | 0 |
| `query-menu` | 3 | 3 | 12 |
| `query-references` | 5 | 5 | 0 |
| `record-input-acceptance` | 6 | 3 | 0 |
| `request-new-conversation` | 23 | 23 | 0 |
| `request-switch-conversation` | 9 | 8 | 0 |
| `resolve-input` | 58 | 58 | 0 |
| `resolve-prompt` | 2 | 1 | 0 |
| `restore-input-attachment` | 1 | 1 | 0 |
| `retain-input` | 14 | 14 | 0 |
| `retain-queued-attachment` | 3 | 3 | 0 |
| `save-draft` | 41 | 41 | 0 |
| `scroll-transcript` | 2 | 2 | 0 |
| `search-transcript` | 1 | 1 | 0 |
| `select-goal` | 1 | 1 | 0 |
| `settle-delegation-success` | 1 | 1 | 0 |
| `settle-failure` | 1 | 1 | 0 |
| `settle-interrupted` | 1 | 1 | 0 |
| `settle-new-conversation` | 22 | 22 | 0 |
| `settle-success` | 11 | 11 | 0 |
| `settle-switch-conversation` | 7 | 7 | 0 |
| `show-chat` | 6 | 6 | 1 |
| `size-transcript` | 1 | 1 | 0 |
| `skip-prompt-question` | 1 | 1 | 0 |
| `submit` | 12 | 12 | 0 |
| `submit-input` | 4 | 4 | 0 |
| `submit-queued` | 7 | 7 | 0 |
| `toggle-changes` | 1 | 1 | 0 |
| `toggle-prompt-option` | 3 | 3 | 0 |
| `view-chat-next` | 3 | 3 | 0 |
| `view-chat-previous` | 1 | 1 | 0 |
| `view-goals-next` | 1 | 1 | 0 |
| `view-goals-previous` | 1 | 1 | 0 |
| `withdraw-usage-row` | 10 | 3 | 0 |

`clear-menu-rows` is dispatched twice by this corpus and applies in neither, but
applies once in the menu corpus.

## Unreachable handler

`inspect` is the one handler of the 135 that no recording applies or
dispatches, because North never sends it (D-80):

- No Rust call site sends it: `rg '"inspect"' src` at the North revision finds
  only a composer test's text and `agent_catalog.rs`'s unrelated subcommand.
- No `handler`, `input handler` or view-handler fact in `north.clause` names
  it, so `resolve-input` and the view keys never map an input to it; it also
  takes no argument while `dispatch_resolved_input` sends one.
- Its clause rewrites a context's phase to itself: it withdraws and includes
  the same phase, so even a dispatch would leave the state unchanged.

A Galileo slice's replay may report `inspect` as unrecorded without a DECISION
line; every other handler has at least one recorded application.
