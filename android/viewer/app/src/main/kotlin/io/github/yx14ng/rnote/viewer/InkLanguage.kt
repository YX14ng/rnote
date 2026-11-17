package io.github.yx14ng.rnote.viewer

/**
 * A language the handwriting is read in, with the tag of its ML Kit model and whether its
 * words are written apart (Chinese and Japanese aren't).
 */
internal enum class InkLanguage(val tag: String, val label: Int, val chip: String, val spaced: Boolean = true) {
    ES("es", R.string.ink_language_es, "ES"),
    EN("en-US", R.string.ink_language_en, "EN"),
    ZH("zh-Hani-CN", R.string.ink_language_zh, "简中", spaced = false),
    ZH_HANT("zh-Hani-TW", R.string.ink_language_zh_hant, "繁中", spaced = false),
    JA("ja", R.string.ink_language_ja, "日本語", spaced = false),
    KO("ko", R.string.ink_language_ko, "한국어"),
    RU("ru", R.string.ink_language_ru, "RU"),
    ;

    companion object {
        /** The language of [tag]; Spanish for an unknown or missing one. */
        fun fromTag(tag: String?): InkLanguage = entries.firstOrNull { it.tag == tag } ?: ES
    }
}
