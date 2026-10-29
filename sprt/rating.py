"""Fit one engine's rating from a gauntlet PGN against anchors of known Elo.

Usage: python3 -I sprt/rating.py <games.pgn> <engine name> <anchors.tsv>

Maximum likelihood under the logistic Elo model with the anchors' ratings
held fixed, counting a draw as half a point. Prints a per-anchor table, then
"rating <elo> <error>" with a 95% interval from the Fisher information. The
estimate is unbounded when the engine wins or loses every game; it then prints
"rating >N" or "rating <N" from the strongest or weakest anchor.
"""

import math
import re
import sys

K = math.log(10) / 400


def expected(diff):
    return 1 / (1 + 10 ** (-diff / 400))


def read_anchors(path):
    anchors = {}
    with open(path) as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            name, _cmd, elo, *_ = line.rstrip("\n").split("\t")
            anchors[name] = float(elo)
    return anchors


def read_games(path, engine):
    """Yield (opponent, engine's score) for each finished game."""
    headers = {}
    with open(path) as f:
        for line in f:
            m = re.match(r'\[(\w+) "(.*)"\]', line)
            if m:
                headers[m.group(1)] = m.group(2)
                continue
            if headers and "Result" in headers and not line.strip():
                result = headers["Result"]
                white, black = headers.get("White"), headers.get("Black")
                headers = {}
                points = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}.get(result)
                if points is None:
                    continue
                if white == engine:
                    yield black, points
                elif black == engine:
                    yield white, 1 - points


def main():
    pgn, engine, anchors_path = sys.argv[1:4]
    anchors = read_anchors(anchors_path)
    games = [(opp, s) for opp, s in read_games(pgn, engine) if opp in anchors]
    if not games:
        sys.exit("no games against known anchors")

    print(f"{'anchor':<10} {'elo':>6} {'games':>6} {'W':>5} {'D':>5} {'L':>5} {'score':>7} {'perf':>6}")
    for name, elo in sorted(anchors.items(), key=lambda a: a[1]):
        scores = [s for opp, s in games if opp == name]
        if not scores:
            continue
        n, total = len(scores), sum(scores)
        w, d = scores.count(1.0), scores.count(0.5)
        frac = total / n
        perf = "n/a"
        if 0 < frac < 1:
            perf = f"{elo + 400 * math.log10(frac / (1 - frac)):.0f}"
        flag = "  (outside 10-90%: weak evidence)" if not 0.1 <= frac <= 0.9 else ""
        print(f"{name:<10} {elo:>6.0f} {n:>6} {w:>5} {d:>5} {n - w - d:>5} {frac:>6.1%} {perf:>6}{flag}")

    total = sum(s for _, s in games)
    if total == 0:
        print(f"rating <{min(anchors[o] for o, _ in games):.0f}")
        return
    if total == len(games):
        print(f"rating >{max(anchors[o] for o, _ in games):.0f}")
        return

    # The likelihood's gradient, sum(score - expected), falls monotonically
    # with the rating, so bisect for its root.
    lo, hi = -1000.0, 5000.0
    for _ in range(100):
        mid = (lo + hi) / 2
        if sum(s - expected(mid - anchors[o]) for o, s in games) > 0:
            lo = mid
        else:
            hi = mid
    rating = (lo + hi) / 2
    info = sum(K * K * expected(rating - anchors[o]) * (1 - expected(rating - anchors[o]))
               for o, _ in games)
    print(f"rating {rating:.0f} {1.96 / math.sqrt(info):.0f}")


if __name__ == "__main__":
    main()
