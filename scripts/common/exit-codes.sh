#!/usr/bin/env bash
# podbox's exit-code table, read out of the binary, for the experiments.
#
# ⭐ TODO/cli.md T-0802. The codes are DATA -- `podbox_probe::exit::CASES`, served
# by `podbox system info --format '{{json .ExitCodes}}'` -- and this is how a
# shell script reads them instead of writing a number down a second time.
#
# ⛔ THE REASON THIS FILE EXISTS IS A MEASURED FAILURE. Six clauses across four
# experiments had `[ "$rc" -eq 2 ]` written into them. On 2026-09-09 T-0802
# measured docker and found that its flag errors are 125 and its post-parse
# refusals are 1, with nothing at 2 at all; every one of those clauses went red
# at once, and each was a second declaration of a value the binary already
# carries. Two clauses of 320-cli-contract.sh had already gone red the same way
# in M4, for naming verbs the parity table carried.
#
# Source it, then use the names:
#
#   . "$REPO/scripts/common/exit-codes.sh"
#   podbox_exit_codes "$BIN" || exit 2
#   [ "$rc" -eq "$PODBOX_EXIT_FLAG_ERROR" ] || fail=1
#
# ⚠ It sets nothing and returns 1 where the binary cannot answer, so a caller
# decides whether that is a skip or a failure. It never falls back to a
# hard-coded number: a default here would be the seventh copy.

# podbox_exit_codes <path-to-podbox>
podbox_exit_codes() {
	local bin="${1:?podbox_exit_codes needs the path to podbox}"
	local doc
	doc="$("$bin" system info --format '{{json .ExitCodes}}' 2>/dev/null)" || return 1
	[ -n "$doc" ] || return 1
	command -v jq >/dev/null 2>&1 || return 1

	local c
	for c in flag-error cli-error runtime-error cannot-invoke not-found no-arguments; do
		local v
		v="$(printf '%s' "$doc" | jq -r --arg c "$c" '.[] | select(.case == $c) | .code')"
		# ⛔ A case the table does not carry is a failure to read it, never a
		# guess: an empty value compared with `-eq` is a syntax error in one
		# clause and a silent pass in another.
		[ -n "$v" ] || return 1
		case "$c" in
		flag-error) PODBOX_EXIT_FLAG_ERROR="$v" ;;
		cli-error) PODBOX_EXIT_CLI_ERROR="$v" ;;
		runtime-error) PODBOX_EXIT_RUNTIME_ERROR="$v" ;;
		cannot-invoke) PODBOX_EXIT_CANNOT_INVOKE="$v" ;;
		not-found) PODBOX_EXIT_NOT_FOUND="$v" ;;
		no-arguments) PODBOX_EXIT_NO_ARGUMENTS="$v" ;;
		esac
	done
	export PODBOX_EXIT_FLAG_ERROR PODBOX_EXIT_CLI_ERROR PODBOX_EXIT_RUNTIME_ERROR
	export PODBOX_EXIT_CANNOT_INVOKE PODBOX_EXIT_NOT_FOUND PODBOX_EXIT_NO_ARGUMENTS
	return 0
}
