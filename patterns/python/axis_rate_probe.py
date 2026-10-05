import sys
sys.path.insert(0, ".")
from world_graph import WorldGraph, ActEdge, ExchangeEdge

g = WorldGraph(means={"m1"}, acts=[
    ActEdge(id="a1", source="A", target="B", category="c", effect={"X": 0.1}, duration_mks=1000.0)])
g.exchanges = [
    ExchangeEdge(id="cheap_slow", gives={"credit": 1.0}, wants={"energy": 10.0}, duration_mks=12_000_000.0),
    ExchangeEdge(id="dear_fast", gives={"credit": 1.0}, wants={"energy": 5.0}, duration_mks=3_000_000.0),
]
r = g.rate("credit", "energy")
print("axis rate credit->energy:", r.rate, "| path:", r.path, "| duration_mks:", r.duration_mks)
print("in the graph G:", sorted(e.id for e in g.exchanges))
print("graph errors:", g.errors())
