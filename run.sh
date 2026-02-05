#!/bin/bash
set -euo pipefail

# --- Logger ---
info() {
    echo "[INFO] $1"
}

warn() {
    echo "[WARN] $1"
}

error() {
    echo "[ERROR] $1" >&2
    exit 1
}

# --- Subcommands ---
plan() {
    info "Executing 'plan' subcommand..."
    if [ -z "$1" ]; then
        error "Objective for 'plan' must be provided."
    fi
    echo "Objective: $1"
    # TODO: Implement planning logic
    # - Use codebase_investigator (via user)
    # - Generate a plan file (e.g., plan.md)
    info "Planning is not yet implemented."
}

work() {
    info "Executing 'work' subcommand..."
    # TODO: Implement work logic
    # - Read plan.md
    # - Create/run tests
    # - Implement code changes
    # - Verify changes
    info "Work is not yet implemented."
}

review() {
    info "Executing 'review' subcommand..."
    # TODO: Implement review logic
    # - Show git diff
    # - Run static analysis
    info "Review is not yet implemented."
}

# --- Main script ---
main() {
    if [ $# -eq 0 ]; Mhit
        error "Usage: $0 <subcommand> [args...]"
    fi

    subcommand=$1
    shift

    case $subcommand in
        plan)
            plan "$@"
            ;;
        work)
            work "$@"
            ;;
        review)
            review "$@"
            ;;
        *)
            error "Unknown subcommand: $subcommand"
            ;;
    esac
}

main "$@"
