-- Balatro globals
globals = {
    "Brainstorm",
    "Controller",
    "Event",
    "G",
    "Game",
    "STR_PACK",
    "STR_UNPACK",
    "UIBox",
    "UIBox_button",
    "attention_text",
    "compress_and_save",
    "copy_table",
    "create_UIBox_round_scores_row",
    "create_option_cycle",
    "create_tabs",
    "create_text_input",
    "create_toggle",
    "darken",
    "get_compressed",
    "number_format",
    "play_sound",
    "random_string"
}

-- LOVE runtime
read_globals = {
    "love"
}

-- Performance: cache all globals
cache = true

-- Allow trailing whitespace (stylua handles this)
ignore = {
    "611", -- trailing whitespace
    "612", -- trailing whitespace in string
    "613", -- trailing whitespace in comment
    "614", -- trailing whitespace in empty line
}

-- Cap ordinary code without rejecting required FFI strings or metadata comments.
max_line_length = false
max_code_line_length = 120
max_string_line_length = false
max_comment_line_length = false

-- Cyclomatic complexity threshold
max_cyclomatic_complexity = 30

-- Exclude the untracked game source.
exclude_files = {
    "BalatroSource/**"
}

files["tests/*.lua"] = {
    globals = { "love" },
    unused_args = false,
}
