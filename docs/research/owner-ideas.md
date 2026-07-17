# Owner idea inventory: phonetics tooling

Ideas about phonetics tooling collected from personal archives outside this
repository, for reference when scoping Phonia features. Sources checked:
Notion workspace, Todoist (active and completed tasks), and a filesystem
search for a Telegram Saved Messages export. Search terms: Praat, phonetic,
phonology, 音声, スペクトログラム, IPA, voice analysis, phonia, phonova, plus
the near variants spectrogram, phonix, waveform, formant, pitch, annotation.

Most hits in each source were unrelated academic notes, coursework records,
or personal journal entries rather than tool ideas; those are not listed.
One Notion page was excluded outright as private personal content (see
"Exclusions" below).

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

- One Notion page was excluded as private personal content unrelated to
  tooling ideas.
- A Todoist project, "漢字音轉換器" (Middle/Old Chinese character-to-sound
  reconstruction converter), has a large backlog (50+ items) of its own, but
  it is a historical-phonology data-conversion tool, not acoustic phonetics
  analysis software, so it was not inventoried beyond the one TTS-related
  item above.
- A Todoist task linking an academic PDF ("2021-yawipa-translations.pdf",
  project "WortWanderer") was left out: it is a bookmarked reference, not a
  stated idea, and its content was not opened to verify what it argues.

## Telegram Saved Messages

A Telegram Desktop export of the owner's Saved Messages (23,982 messages,
2017–2026) was located and its `result.json` scanned for tooling ideas, using
the same kind of term list as the other sources (praat, phonetic, phonology,
音声, 音韻, スペクトログラム, フォルマント, IPA, formant, pitch, spectrogram,
annotation, アノテーション, voice, 声, TextGrid, ELAN, alignment, 転写,
transcription, tone, アクセント, prosody, plus a supplementary pass for tts,
読み上げ, wav, recording, corpus). Saved Messages is a personal scratchpad, so
most hits were song lyrics, dictionary lookups, casual chat, coursework
notes, and general corpus-linguistics/NLP research jottings unrelated to a
tool or feature; those are not itemized. Content unrelated to tooling was not
inventoried. A couple of hits echoed a third party's pasted text inline
(a quoted chat message, a correspondent's sign-off); only the owner's own
idea sentence from those messages is reflected below, with the third-party
material dropped.

### Analysis features

- Bookmarked references for converting a WAV file to a spectrogram in Python
  (via `pydub`), saved right alongside Electron app documentation — read
  together as reconnaissance for a desktop spectrogram-rendering feature.
  (saved note, 2020-03-20)
- "flashlight for phonetic imaging" — a technique note about using a
  flashlight as a light source for phonetic/articulatory imaging. (saved
  note, 2023-08-11)
- "Allow non praat input (recording)" — accept a live microphone recording as
  an input method, rather than requiring pre-existing Praat-format audio.
  (saved note, 2025-11-29)
- "Use ai to auto get standard vowel" — have the tool infer a user's
  reference/standard vowel automatically from a recording, instead of asking
  them to specify it by hand. (saved note, 2025-11-30)
- A calibration-dialogue flow: "Make so that when user access standard
  recording, give a dialogue to force user to choose record your standard
  vowels first to give accurate result, or use existing profile, and let user
  remember my choice, and add a ask option to be able to show the dialogue
  again." (saved note, 2025-12-01)

### Annotation

- "web annotations vs. inline markup" — a design-comparison note weighing an
  external, web-annotation-style layer against inline markup for an
  annotation format. (saved note, 2023-09-30)
- "アイヌ語の登場人物の検出・アノテーション" (character detection and
  annotation for Ainu-language text) — automatically detect
  characters/entities in a text and annotate them. (saved note, 2024-09-22)
- "簡易品詞検索　アノテーション" (simple part-of-speech search, annotation).
  (saved note, 2024-12-12)
- "partial annotation" — support annotating only part of a text or recording,
  rather than requiring a full pass. (saved note, 2024-12-13)
- "手動 annotation 補助" (manual-annotation assistance) — tooling to assist a
  person doing annotation by hand. (saved note, 2024-12-29)

### AI / auto-labeling

- "アクセント自動付けツール" (automatic pitch-accent labeling tool), saved a
  few days after bookmarking an online "accent guesser" web tool as a
  reference point. (saved note, 2024-12-20 / 2024-12-17)

### Voice training / pronunciation output

- To set a transliteration policy for loanwords entering a language, first
  build a chronologically dated loanword database as the basis for the
  guidelines. (saved note, 2024-03-03)
- Standardize a katakana-based text database first, then run it through
  automatic speech synthesis to produce near-accurate synthetic speech —
  written as a case for designing the orthography with that downstream
  synthesis step already in mind. (saved note, 2024-05-13)
- "アイヌ語の音声を合成したり取ってきたりするやつ　単語読み上げ" (something
  that synthesizes or fetches speech for a language — word read-aloud).
  (saved note, 2024-11-27)
- A pipeline-tool sketch chaining source script through original-form lookup,
  data, transcription, speech, morpheme, and meaning stages ("訓民正音→原点→
  資料→転写→音声→形態素→意味　の流れのツール"). (saved note, 2024-12-03)
- A ranked comparison of text-to-speech engines for a read-aloud feature:
  "VOICEPEAK > VOICEVOX ~ AIVISSPEECH". (saved note, 2024-12-03)
- "allow voiced in ainconv" — feature request for an existing
  language-conversion tool to support voiced consonants. (saved note,
  2024-12-22)
- Bookmarked reference to a text-to-speech tool built on a historical
  phonological reconstruction system. (saved note, 2025-11-11)
- Bookmarked reference to an online voice-gender analyzer tool. (saved note,
  2026-04-01)
