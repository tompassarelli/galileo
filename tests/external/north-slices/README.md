# North slice transition reference

This corpus records the 33 North tests named by PLAN boxes 8.13.1r to 8.13.20r
(PLAN box 8.13.0, DECISIONS D-74) at North revision
`eaee1838d1968b7caba65195866e5d52fd30b2e1`, using the recorder commit below.
`restart_storage_fresh_process` is named by both 8.13.2r and 8.13.9r and is
recorded once. The Clause compiler/runtime pin is
`258e6956aa11dab2e724e89439e864e5e5dafdd6` (MIT OR Apache-2.0). Only public
Clause APIs were used; no Clause source is copied here.

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
`rescue/8.5-recorder-2daaad3`. It was run from
`~/code/north/worktrees/galileo-slice-recorder` on branch
`galileo-slice-recorder`, unmerged and unpublished, for box 8.13.0b.

Measured on 2026-10-12. The 33 recorded tests passed in 5.4 s to 16.1 s each.
Admissions count occurrence admissions, Effects the per-transaction effect
records, and Handlers the distinct handler names dispatched in that test.

| Test | Sessions | Transactions | Admissions | Effects | Projections | Handlers | Bytes |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `switching_running_conversations_keeps_drafts_and_interrupts_only_the_selected_turn` | 1 | 50 | 371 | 50 | 50 | 33 | 3,763,247 |
| `clause_owns_new_and_switched_conversation_identity` | 1 | 7 | 51 | 7 | 7 | 13 | 511,972 |
| `saved_workspace_handles_navigation_while_reconnection_is_pending` | 1 | 6 | 45 | 6 | 6 | 15 | 436,031 |
| `restart_storage_fresh_process` | 2 | 27 | 195 | 27 | 27 | 16 | 2,011,681 |
| `usage_display_distinguishes_missing_values_and_preserves_conversation_scope` | 1 | 10 | 85 | 10 | 10 | 17 | 863,794 |
| `reference_catalog_can_be_replaced_after_a_full_listing` | 1 | 3 | 535 | 3 | 3 | 10 | 1,596,065 |
| `escape_closes_references_and_typing_reopens_them` | 1 | 4 | 32 | 4 | 4 | 11 | 296,441 |
| `chat_batch_preserves_delta_order_and_final_replacement` | 1 | 5 | 38 | 5 | 5 | 11 | 376,820 |
| `transcript_controls_scroll_filter_changes_and_copy_without_changing_the_draft` | 1 | 28 | 196 | 28 | 28 | 12 | 2,451,453 |
| `approval_uses_only_offered_decisions_and_retains_exact_request_identity` | 1 | 14 | 107 | 14 | 14 | 19 | 1,057,009 |
| `multiple_questions_accept_choices_and_free_text_without_crossing_threads` | 1 | 13 | 101 | 13 | 13 | 19 | 1,050,193 |
| `tool_forms_use_checked_choices_validate_fields_and_keep_the_chat_draft` | 1 | 20 | 156 | 20 | 20 | 21 | 1,775,449 |
| `cancellation_is_explicit_and_unknown_delivery_never_resends` | 1 | 9 | 68 | 9 | 9 | 17 | 686,559 |
| `running_input_becomes_steering_and_failed_delivery_stays_unsent` | 1 | 12 | 89 | 12 | 12 | 22 | 828,926 |
| `queued_inputs_wait_for_settlement_and_keep_their_own_attachments` | 1 | 19 | 147 | 19 | 19 | 25 | 1,362,996 |
| `steering_waits_for_receipts_and_expiry_never_retargets_a_new_turn` | 1 | 17 | 128 | 17 | 17 | 22 | 1,191,367 |
| `conversation_turns_queues_and_drafts_are_independent_of_selection` | 1 | 22 | 165 | 22 | 22 | 26 | 1,577,240 |
| `clause_owns_submit_and_settlement` | 1 | 3 | 23 | 3 | 3 | 11 | 224,128 |
| `clause_owns_delegation_admission_and_settlement` | 1 | 5 | 37 | 5 | 5 | 12 | 356,229 |
| `clause_owns_delegation_failure_before_and_after_spawn` | 2 | 7 | 53 | 7 | 7 | 13 | 514,524 |
| `clause_owns_image_identity_draft_rollover_and_submission_membership` | 1 | 7 | 54 | 7 | 7 | 15 | 491,605 |
| `clause_rejects_unknown_switch_and_preserves_active_conversation_on_failure` | 1 | 7 | 50 | 7 | 7 | 13 | 503,817 |
| `slash_popup_exposes_the_basic_command_surface` | 1 | 1 | 7 | 1 | 1 | 7 | 91,774 |
| `checkpoint_failure_does_not_release_a_host_effect` | 2 | 5 | 37 | 5 | 5 | 13 | 420,035 |
| `spec_6_work_commands_route_without_a_coding_turn` | 1 | 25 | 175 | 25 | 25 | 10 | 1,676,630 |
| `history_commands_require_archive_confirmation_and_pause_archived_work` | 1 | 23 | 165 | 23 | 23 | 24 | 1,611,699 |
| `usage_compaction_waits_for_completion_without_requiring_an_answer` | 1 | 16 | 119 | 16 | 16 | 25 | 1,160,536 |
| `local_settings_effect_is_saved_with_its_completed_menu` | 2 | 6 | 51 | 6 | 6 | 20 | 504,993 |
| `clause_creates_orders_selects_and_edits_goals_with_revisions` | 1 | 24 | 168 | 24 | 24 | 14 | 1,659,271 |
| `clause_drives_the_tui_goal_create_inspect_edit_inspect_journey` | 1 | 19 | 139 | 19 | 19 | 16 | 1,388,083 |
| `inline_goal_commands_edit_and_clear_without_submitting_a_turn` | 1 | 19 | 139 | 19 | 19 | 11 | 1,410,969 |
| `goals_view_renders_its_empty_state` | 1 | 3 | 21 | 3 | 3 | 9 | 223,989 |
| `relation_decode_reuses_unchanged_history_and_observes_replacement` | 1 | 4 | 28 | 4 | 4 | 9 | 1,200,686 |

The corpus is 66 files totalling 35,276,211 bytes (33.64 MiB), under the 64 MB bound.

## Handler coverage

`north.clause` at the North revision declares 135 handler names (`on NAME`
lines, 150 clauses). The union of the 33 recordings dispatches
104 of them; dispatch counts are summed over every session.

| Handler | Dispatches |
| --- | ---: |
| `abandon-turn` | 0 |
| `accept-menu` | 0 |
| `answer-prompt-choice` | 4 |
| `answer-prompt-multiple` | 1 |
| `answer-prompt-text` | 4 |
| `append-chat` | 38 |
| `attach-image` | 10 |
| `begin-conversation-reconciliation` | 1 |
| `begin-reconnect` | 0 |
| `bind-input-client-id` | 2 |
| `cancel-prompt` | 1 |
| `child-spawned` | 2 |
| `clear-chat` | 0 |
| `clear-host-effect` | 28 |
| `clear-menu-rows` | 1 |
| `clear-references` | 4 |
| `clear-resolved-answers` | 422 |
| `clear-submitted-attachment` | 2 |
| `clear-turn` | 16 |
| `close-menu` | 0 |
| `close-turn-prompts` | 7 |
| `command-agents` | 0 |
| `command-archive` | 3 |
| `command-archived` | 0 |
| `command-compact` | 1 |
| `command-config` | 2 |
| `command-delegate` | 0 |
| `command-effort` | 0 |
| `command-fork` | 1 |
| `command-goal` | 12 |
| `command-goals` | 2 |
| `command-model` | 0 |
| `command-new` | 0 |
| `command-quit` | 0 |
| `command-reconnect` | 0 |
| `command-rename` | 1 |
| `command-resume` | 0 |
| `command-usage` | 1 |
| `command-work` | 8 |
| `compaction-completed` | 1 |
| `connection-lost` | 4 |
| `copy-queued-to-submitted` | 1 |
| `copy-submission-attachment` | 2 |
| `count-menu` | 1 |
| `count-usage` | 422 |
| `delegate` | 4 |
| `delegate-task` | 0 |
| `detach-image` | 3 |
| `disconnect-prompts` | 4 |
| `dismiss-references` | 1 |
| `ensure-context` | 31 |
| `expire-steering` | 16 |
| `fail-delegation-after-child` | 1 |
| `fail-delegation-before-child` | 1 |
| `fail-new-conversation` | 0 |
| `fail-switch-conversation` | 1 |
| `filter-menu` | 0 |
| `filter-transcript` | 422 |
| `finish-observed-turn` | 7 |
| `finish-reconnect` | 1 |
| `focus-prompts` | 422 |
| `forget-input` | 3 |
| `forget-input-attachment` | 0 |
| `goal-edit-input` | 3 |
| `goal-objective` | 3 |
| `goal-title` | 3 |
| `history-action-completed` | 0 |
| `initialize` | 33 |
| `input-receipt` | 5 |
| `inspect` | 0 |
| `move-menu` | 0 |
| `move-reference` | 0 |
| `navigate-prompt` | 4 |
| `observe-archived` | 1 |
| `observe-chat-item` | 7 |
| `observe-context-usage` | 2 |
| `observe-conversation` | 3 |
| `observe-input-acceptance` | 2 |
| `observe-prompt` | 6 |
| `observe-prompt-option` | 21 |
| `observe-prompt-question` | 13 |
| `observe-quota-window` | 4 |
| `observe-reset-credits` | 2 |
| `observe-restored` | 1 |
| `observe-settings` | 4 |
| `observe-stored-turn` | 0 |
| `observe-turn` | 8 |
| `offer-history-row` | 0 |
| `offer-reference` | 514 |
| `offer-settings-row` | 2 |
| `offer-work-row` | 0 |
| `open-history-menu` | 0 |
| `open-settings-menu` | 1 |
| `pause-disconnected-inputs` | 4 |
| `prepare-input-edit` | 0 |
| `prepare-prompt-response` | 8 |
| `prepare-queued-input` | 10 |
| `prepare-steering` | 6 |
| `present-prompt-questions` | 422 |
| `present-usage` | 422 |
| `prompt-answer-problem` | 3 |
| `prompt-response-written` | 2 |
| `query-menu` | 1 |
| `query-references` | 4 |
| `record-input-acceptance` | 5 |
| `request-new-conversation` | 16 |
| `request-switch-conversation` | 9 |
| `resolve-input` | 43 |
| `resolve-prompt` | 2 |
| `restore-input-attachment` | 0 |
| `retain-input` | 10 |
| `retain-queued-attachment` | 1 |
| `save-draft` | 33 |
| `scroll-transcript` | 2 |
| `search-transcript` | 1 |
| `select-goal` | 1 |
| `settle-delegation-success` | 1 |
| `settle-failure` | 1 |
| `settle-interrupted` | 1 |
| `settle-new-conversation` | 16 |
| `settle-success` | 11 |
| `settle-switch-conversation` | 7 |
| `show-chat` | 4 |
| `size-transcript` | 1 |
| `skip-prompt-question` | 1 |
| `submit` | 10 |
| `submit-input` | 3 |
| `submit-queued` | 5 |
| `toggle-changes` | 1 |
| `toggle-prompt-option` | 3 |
| `view-chat-next` | 1 |
| `view-chat-previous` | 0 |
| `view-goals-next` | 0 |
| `view-goals-previous` | 0 |
| `withdraw-usage-row` | 10 |

These 31 handlers are never dispatched by the corpus:

- `abandon-turn`
- `accept-menu`
- `begin-reconnect`
- `clear-chat`
- `close-menu`
- `command-agents`
- `command-archived`
- `command-delegate`
- `command-effort`
- `command-model`
- `command-new`
- `command-quit`
- `command-reconnect`
- `command-resume`
- `delegate-task`
- `fail-new-conversation`
- `filter-menu`
- `forget-input-attachment`
- `history-action-completed`
- `inspect`
- `move-menu`
- `move-reference`
- `observe-stored-turn`
- `offer-history-row`
- `offer-work-row`
- `open-history-menu`
- `prepare-input-edit`
- `restore-input-attachment`
- `view-chat-previous`
- `view-goals-next`
- `view-goals-previous`
