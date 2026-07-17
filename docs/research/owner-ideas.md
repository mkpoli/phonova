# Owner idea inventory: phonetics tooling

Ideas about phonetics tooling collected from personal archives outside this
repository, for reference when scoping Phonia features. Sources checked:
Notion workspace, Todoist (active and completed tasks), and a filesystem
search for a Telegram Saved Messages export. Search terms: Praat, phonetic,
phonology, 音声, スペクトログラム, IPA, voice analysis, phonia, phonova, plus
the near variants spectrogram, phonix, waveform, formant, pitch, annotation.

Most hits in each source were unrelated academic notes, coursework records,
or personal journal entries rather than tool ideas; those are not listed.
One Notion page containing private personal content was excluded
outright (see "Exclusions" below).

## Analysis features

- Render a spectrogram via FFT — noted as a build step while working on an
  unrelated waveform-comparison video generator, not a phonetics project
  proper, but the computation is the same one a phonetics analysis view
  needs. (Todoist, project "音樂分割比較視頻生成器", task "FFT and draw
  spectrogram", https://app.todoist.com/app/task/63rC7367Qh78h2mv)

## UX

- User-selectable font for IPA glyph rendering, so users can pick a display
  font for phonetic transcription rather than being locked to one. (Todoist,
  project "世界整合語源システム", task "用戶級 IPA 字體選擇功能",
  https://app.todoist.com/app/task/63rC7CV65h7GjCMx)
- A dedicated IPA input tool for typing phonetic transcription without a
  full keyboard layout switch. (Todoist, project "輸入法", task "IPA 輸入工具",
  https://app.todoist.com/app/task/63r8qjWqqFgHvg2v)
- IPA handwriting input method (draw a symbol, get the character) as an
  alternative to a picker or keyboard layout. (Todoist, project "輸入法",
  task "國際音標手寫輸入法", https://app.todoist.com/app/task/654XJwHqfP23P2RM)

## Mobile

- Make the tool usable from a phone or iPad, not desktop-only. (Todoist,
  project "アイヌ語", task "スマホ・iPadからもアクセス可能に",
  https://app.todoist.com/app/task/69Fcj7HgVxFRgQQM)

## AI / auto-labeling

- Automatically flag dictionary/database entries where the IPA field is
  still a placeholder (e.g. an ellipsis) instead of a real transcription, as
  part of a general invalid-entry sweep. (Todoist, project "アイヌ語", task
  "IPAがテンテンテンの項目" under parent task "自動的に invalid 項目を洗い出す",
  https://app.todoist.com/app/task/69JHHmX9xqQh9rJM)

## Voice training / pronunciation output

- Find or build an IPA-driven text-to-speech engine to back a read-aloud
  feature, i.e. synthesize speech directly from a phonetic transcription
  rather than from spelling. (Todoist, project "漢字音轉換器", task "找到
  IPA TTS" under parent task "朗讀功能",
  https://app.todoist.com/app/task/63r9pWh9PCWCj6W6)

## Pedagogy / research use case

- A 2022 thesis-topic brainstorm proposed using Praat to measure pitch for
  an accent/intonation study of long numeral strings (e.g. phone numbers),
  with a stretch goal of cross-language comparison. This is a research
  design note rather than a software feature request, but it implies a
  concrete analysis need: pitch-contour extraction and comparison across an
  arbitrary-length token sequence, replicated across speakers or dialects.
  (Notion, page "卒論題目", https://app.notion.com/p/cc7ed7699755452da2bfe0505892dbeb)

## Integrations

None found. No owner notes tied phonetics tooling to a specific external
service or API integration.

## Exclusions

- A Notion page on private personal content
  excluded outright as
  sensitive personal content unrelated to tooling ideas.
- A Todoist project, "漢字音轉換器" (Middle/Old Chinese character-to-sound
  reconstruction converter), has a large backlog (50+ items) of its own, but
  it is a historical-phonology data-conversion tool, not acoustic phonetics
  analysis software, so it was not inventoried beyond the one TTS-related
  item above.
- A Todoist task linking an academic PDF ("2021-yawipa-translations.pdf",
  project "WortWanderer") was left out: it is a bookmarked reference, not a
  stated idea, and its content was not opened to verify what it argues.

## Telegram Saved Messages

No Telegram export was found. Searched `~/Downloads`, `~/Documents`,
`~/<projects>`, `~/projects` (none of the first two exist on this machine) and,
since this is WSL2, the mounted Windows profile
`/mnt/c/Users/<user>/{Downloads,Documents,My Documents}` for `ChatExport`
directories, `result.json`, and `messages*.html` (Telegram Desktop's export
formats). The only Telegram-named folders found ("Telegram BOT" under the
Windows Documents folders) contain bot-development source code
(`mkpoli_bot`, `shiritori_bot`, `sandbox_bot.py`), not a Saved Messages
export. This source yielded nothing and was not searched further.
