# Deck outline — binmap, for a YouTube segment aimed at Rust devs

Audience: working developers watching a video. Goal: use it, then contribute.
Assumed length: 13 slides, roughly 10 minutes of talking.
Title style: short topic noun-phrases (no verdict titles, no punchlines).

1.  binmap — source-aware binary analysis for Rust     (title, status stated)
2.  The gap between source and binary                  (two columns, mono examples)
3.  Three questions, one engine                        (three blocks)
4.  What the engine measures                           (table: tool → what it gives)
5.  Measured, derived, inferred                        (three large glyphs)
6.  Evidence in the interface                          (product figure, DS components at 2x)
7.  Trust tiers                                        (four steps, Propose default)
8.  The verification harness                           (gate list, noise floor)
9.  Targets and capabilities                           (three columns, ✓ / ✗)
10. Non-goals                                          (six items, scope-creep line)
11. Status and phases                                  (phase list, weeks)
12. Where contributions land                           (three areas)
13. What the project needs                             (close, link in description)

Reading the titles alone: the gap → what it answers → how it measures → how it
labels claims → what that looks like on screen → how much it is allowed to do →
what it must pass → what it supports → what it refuses → where it is → where to
help → what is needed.

Backgrounds: --surface-app for most, --surface-panel for 4, 8, 10, 12. No third colour.
Orange: one thing per slide — the eyebrow, or the accent number, or inferred provenance.
Type: 64 title / 44 subtitle / 34 body / 28 small / 24 caps. Padding 100/100/80.
