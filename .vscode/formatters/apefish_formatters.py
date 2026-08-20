# CodeLLDB pretty-printers for apefish-engine's board types.
#
# CodeLLDB shows raw memory layout for user types by default (it doesn't call
# Rust's `Debug` impls). These summary providers give Square, Bitboard, and
# the PieceKind family the same human-readable formatting as their Debug
# impls in engine/src/basetypes.rs. Registered via launch.json's
# "initCommands".

SQUARE_FILES = "abcdefgh"

PIECE_KIND_NAMES = ["Pawn", "Knight", "Bishop", "Rook", "Queen", "King"]
INDEXED_PIECE_KIND_NAMES = ["Knight", "Bishop", "Rook", "King"]
SLIDING_PIECE_KIND_NAMES = ["Bishop", "Rook"]


def _square_name(value):
    file = SQUARE_FILES[value % 8]
    rank = (value // 8) + 1
    return f"{file}{rank}"


def _named(names, value):
    return names[value] if 0 <= value < len(names) else f"<unknown:{value}>"


def square_summary(valobj, internal_dict):
    return _square_name(valobj.GetValueAsUnsigned())


def bitboard_summary(valobj, internal_dict):
    bits = valobj.GetChildMemberWithName("0").GetValueAsUnsigned()
    squares = [_square_name(i) for i in range(64) if bits & (1 << i)]
    return "[" + ", ".join(squares) + "]"


def piece_kind_summary(valobj, internal_dict):
    return _named(PIECE_KIND_NAMES, valobj.GetValueAsUnsigned())


def indexed_piece_kind_summary(valobj, internal_dict):
    value = valobj.GetValueAsUnsigned()
    return f"{_named(INDEXED_PIECE_KIND_NAMES, value)} (index {value})"


def sliding_piece_kind_summary(valobj, internal_dict):
    return f"{_named(SLIDING_PIECE_KIND_NAMES, valobj.GetValueAsUnsigned())} (sliding)"


def __lldb_init_module(debugger, internal_dict):
    category = debugger.CreateCategory("apefish")
    category.SetEnabled(True)

    formatters = {
        r"^apefish_engine::basetypes::Square$": "square_summary",
        r"^apefish_engine::basetypes::Bitboard$": "bitboard_summary",
        r"^apefish_engine::basetypes::PieceKind$": "piece_kind_summary",
        r"^apefish_engine::basetypes::IndexedPieceKind$": "indexed_piece_kind_summary",
        r"^apefish_engine::basetypes::SlidingPieceKind$": "sliding_piece_kind_summary",
    }
    for regex, func_name in formatters.items():
        debugger.HandleCommand(
            f'type summary add -x "{regex}" -F {__name__}.{func_name} --category apefish'
        )
