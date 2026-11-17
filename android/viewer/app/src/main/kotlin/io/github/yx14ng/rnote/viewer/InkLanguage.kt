package io.github.yx14ng.rnote.viewer

/** A language the handwriting is read in, with the tag of its ML Kit model. */
internal enum class InkLanguage(val tag: String, val label: Int, val chip: String) {
    ES("es", R.string.ink_language_es, "ES"),
    EN("en-US", R.string.ink_language_en, "EN"),
    ZH("zh-Hani-CN", R.string.ink_language_zh, "中文"),
    ;

    companion object {
        /** The language of [tag]; Spanish for an unknown or missing one. */
        fun fromTag(tag: String?): InkLanguage = entries.firstOrNull { it.tag == tag } ?: ES
    }
}
