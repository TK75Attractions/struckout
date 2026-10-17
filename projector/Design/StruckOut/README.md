# STRUCK OUT projector design kit

The supplied `reference_logo.jpg` defines this kit's visual language: a black field, sharp white geometry, electric blue and bright yellow slashes. Images were generated with the built-in ChatGPT image tool from that reference.

## Deliverables

| File | Size | Use |
| --- | --- | --- |
| `../../Assets/Art/StruckOut/arena_background.png` | 1672 × 941 | Reusable playfield backdrop. Keep the center clear for dynamically positioned targets. |
| `../../Assets/Art/StruckOut/circle_target.png` | 1254 × 1254, transparent | Single circular target sprite. Its visible diameter almost fills its canvas so Unity's sprite width matches the collision diameter. |
| `../../Assets/Art/StruckOut/hit_scored.png` | 1254 × 1254, transparent | Bright scoring impact. |
| `../../Assets/Art/StruckOut/hit_missed.png` | 1254 × 1254, transparent | Quiet diamond miss marker. |
| `../../Assets/Art/StruckOut/hit_ignored.png` | 1254 × 1254, transparent | Inactive-session impact marker. |
| `idle_concept.png` / `../../Assets/Art/StruckOut/idle_screen.png` | 1672 × 941 | Projector standby composition with the logo, imported as the live idle screen. |
| `playing_concept.png` | 1672 × 941 | Example of four targets on the field. The actual target positions are generated at runtime. |
| `finished_concept.png` / `../../Assets/Art/StruckOut/finished_screen.png` | 1672 × 941 | Finished-state composition with a clear area for live text, imported as the live finish screen. |

The original logo is kept separately as `reference_logo.jpg`. Targets, impact effects, and the finish message remain Unity objects above the screen artwork.

## Unity animation integration

- The board is logically 1920 × 1080. `UIService` stretches each screen sprite to that board, with the background behind the runtime targets and the idle/finish screens above them.
- `GameRuntime` drives Idle → Playing → Finished. `UIService` crossfades the screens over 0.55 seconds; the circle targets fade in with short staggered delays and fade out at finish.
- `CircleTargetUI` preserves the target diameter while adding a subtle rotation and a short ease/squash during hit relocation.
- A hit keeps the original expanding ripple: a thin cyan ring expands from the target's old position and blooms again at its new position. The target continues its existing eased move, while the separate ripple remains behind the new impact artwork.
- `CollisionResult.Scored`, `Missed`, and `Ignored` select different transparent impact sprites. `CollisionMarker` expands, turns where appropriate, fades, and removes itself.
- The finish screen's `TIME UP` text is a Unity `TextMesh` so its wording can be edited. The touchpanel already shows time, score, and the result.
- `Assets/Editor/StruckOutAnimationSetup.cs` supplies **Tools → STRUCK OUT → Apply Projector Animation Art** to reimport the seven live sprites and assign them to the scene and target prefab after artwork changes.
- To preview without the physical tracker or game master, set `GameLifetimeScope` → `Network Settings` → `Mode` to `Fake` in the Inspector and enter Play Mode. The fake services start the game, send collisions, and finish it after 150 seconds. Restore `Real` for the installation.

## Reusable image prompt set

**Shared reference:** Use `reference_logo.jpg` only as a brand/style reference. Match its near-black field, white geometric shapes, electric blue and fluorescent yellow diagonal strokes. Use flat 2D, sharp edges and high projector contrast. No baseball illustrations, textures, gradients, watermarks or UI buttons.

1. **Playing composition:** Full 16:9 projected game field. Four fully visible circular targets of varied sizes, widely spaced. Sparse blue/yellow/white edge frame, quiet black center. No words, score or timer.
2. **Background plate:** The same 16:9 edge frame and pure black central field, without targets, text or other central graphics.
3. **Target sprite:** One perfectly centered circular target on true transparency. Thick segmented white ring, small blue/yellow ring segments, black interior, white bullseye and short diagonal wedges clipped inside the exact circular perimeter. Circle tangent to all four canvas edges; no padding or external glow.
4. **Scored impact:** One transparent centered burst of white pinpoint, broken blue/yellow rings and diagonal shards. Bright enough to read over the target; no words.
5. **Missed impact:** One transparent centered white/blue hollow diamond with four small diagonal ticks. Clearly quieter than scored impact; no circular burst.
6. **Ignored impact:** One transparent compact white angular bracket with a yellow diagonal slash. Neutral, visually distinct from scored and missed impacts.
7. **Idle composition:** The supplied STRUCK OUT logo large and centered on the black arena, with faint dormant ring hints. Leave blank space for an editable Unity instruction; add no new text.
8. **Finished composition:** Same frame with dimmed target-ring hints and a bold empty central title zone outlined by white, blue and yellow brackets. Leave all words and score to Unity.
