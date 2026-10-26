# Oxanium

`Oxanium-VariableFont_wght.ttf` is copied unchanged from
`touchpanel/ui/assets/fonts/Oxanium-VariableFont_wght.ttf` so the touchpanel and
projector use the same typeface. When updating the touchpanel font, update this
copy as well. `UIService._displayFont` in `SampleScene` references this asset.

The font is used for the playing HUD and numeric scores on result and ranking screens.
Japanese labels and player names use the bundled Noto Sans JP asset below.

The font is distributed under the SIL Open Font License 1.1; see `OFL.txt`.
License source: https://github.com/google/fonts/blob/main/ofl/oxanium/OFL.txt

# Noto Sans JP

`NotoSansJP.ttf` provides Japanese screen labels and player names. It is copied from
[google/fonts](https://github.com/google/fonts/tree/66a36c8c94b1a5d992ee4e7f392fccfe4945767c/ofl/notosansjp),
revision `66a36c8c94b1a5d992ee4e7f392fccfe4945767c`, with the SIL Open Font License in `NotoSansJP-OFL.txt`.
The existing Oxanium remains the typeface for scores and the playing HUD.
