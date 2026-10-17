# Anytype UI reference

The settings layout follows the official [Anytype desktop client](https://github.com/anyproto/anytype-ts) at revision `4b265d9564d70a45b030cfb64669b4508eb915ef`. The components remain local HTML and CSS so the Tauri settings app does not need Anytype's React runtime.

The following upstream files supplied the measurements and control behavior.

- [Settings layout](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/page/main/settings.scss) sets the 640px content limit, 28px page title, 16px settings rows, 14px/16px row padding, and 12px group corners.
- [Settings sidebar](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/component/sidebar/page/settings.scss) sets compact navigation rows, 20px icons, and 12px group labels.
- [Buttons](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/button.scss) use a 40px height, 20px corner radius, 14px type, and neutral disabled state.
- [Inputs](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/input.scss) supply the filled backgrounds and subtle hover/focus highlight. The text fields use the user's Anytype screenshot measurements of 48px height and 18px corners.
- [Switches](https://github.com/anyproto/anytype-ts/blob/4b265d9564d70a45b030cfb64669b4508eb915ef/src/scss/form/switch.scss) use a 40×24px track and a 20px thumb in their large variant.

Colors and font sizes also match the installed Anytype stylesheet. BlobUI's saved interface and code fonts still apply. The custom color picker and explicit Save action remain BlobUI controls. Decorative shadows, glows, theme cards, and font samples remain absent.

Scrollbars follow the user's Anytype screenshot, with a 12px width, rounded thumb ends, no arrow buttons, and a transparent track. Their gray thumbs match the current theme, appear while scrolling, and fade after scrolling stops. Scroll areas without overflow show no scrollbar.
