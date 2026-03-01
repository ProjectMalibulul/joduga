#!/usr/bin/env python3
"""Feedback collector for the Joduga UI review loop.

Run this after launching the Joduga UI.  Type your feedback and press Enter.
Type 'ok', 'done', 'looks good', or 'lgtm' to signal everything is fine.
Type 'quit' or 'exit' to abort.
"""
import sys

def main():
    print("=" * 50)
    print("  Joduga UI — Feedback Collector")
    print("=" * 50)
    print()
    print("The Joduga UI window should now be open.")
    print("Please interact with it and provide feedback below.")
    print()
    print("  • Type your feedback and press Enter.")
    print("  • Type 'ok' / 'done' / 'lgtm' if everything looks good.")
    print("  • Type 'quit' / 'exit' to abort.")
    print()

    while True:
        try:
            feedback = input("Your feedback> ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\n[Aborted]")
            sys.exit(1)

        if not feedback:
            continue

        # Echo for the agent to read
        print(f"[FEEDBACK] {feedback}")

        lower = feedback.lower()
        if lower in ("ok", "done", "looks good", "lgtm", "good", "yes", "perfect", "all good"):
            print("[STATUS] APPROVED")
            sys.exit(0)
        elif lower in ("quit", "exit", "abort", "stop"):
            print("[STATUS] ABORTED")
            sys.exit(1)
        else:
            # Feedback contains issues — the agent will read this
            print("[STATUS] NEEDS_CHANGES")
            sys.exit(2)

if __name__ == "__main__":
    main()
