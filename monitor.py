import os
import json
import time
import subprocess
from collections import deque
import requests
from typing import Optional, Dict, Any

from textual.app import App, ComposeResult
from textual.widgets import Header, Footer, Static, DataTable, RichLog
from textual.containers import Vertical, Horizontal, Container
from textual.timer import Timer
from textual_plotext import PlotextPlot

# Configuration
LLAMA_API_URL = "http://127.0.0.1:8086"
REFRESH_RATE = 2.0  # Hz
HISTORY_SECONDS = 60
MAX_BUFFER = int(HISTORY_SECONDS * REFRESH_RATE)

class TelemetryEngine:
    def __init__(self):
        self.session = requests.Session()
        self.session.headers.update({"Connection": "keep-alive"})
        self.last_llama_counts = {} # slot_id -> count

    def get_gpu_data(self) -> Optional[Dict[str, Any]]:
        try:
            result = subprocess.run(['rocm-smi', '-a', '--json'], capture_output=True, text=True)
            if result.returncode != 0: return None
            card = json.loads(result.stdout).get('card0', {})
            
            mem_info = subprocess.run(['free', '-b'], capture_output=True, text=True).stdout.splitlines()[1].split()
            
            return {
                "usage": float(card.get("GPU use (%)", 0)),
                "temp": float(card.get("Temperature (Sensor edge) (C)", 0)),
                "power": float(card.get("Current Socket Graphics Package Power (W)", 0)),
                "mem_total": int(mem_info[1]) / (1024**3),
                "mem_used": int(mem_info[2]) / (1024**3)
            }
        except Exception:
            return None

    def get_inference_data(self) -> Optional[Dict[str, Any]]:
        try:
            slots = self.session.get(f"{LLAMA_API_URL}/slots", timeout=1.0).json()
            props = self.session.get(f"{LLAMA_API_URL}/props", timeout=1.0).json()
            
            processed_slots = []
            now = time.time()
            
            for s in slots:
                sid = s['id']
                decoded = s.get('next_token', [{}])[0].get('n_decoded', 0) if isinstance(s.get('next_token'), list) else 0
                
                # Simple TPS calculation within the engine
                prev = self.last_llama_counts.get(sid, {"count": decoded, "time": now})
                dt = now - prev["time"]
                tps = (decoded - prev["count"]) / dt if dt > 0 and decoded > prev["count"] else 0.0
                self.last_llama_counts[sid] = {"count": decoded, "time": now}

                processed_slots.append({
                    "id": sid,
                    "busy": s['is_processing'],
                    "task": s.get('id_task', 'N/A'),
                    "decoded": decoded,
                    "remain": s.get('next_token', [{}])[0].get('n_remain', 0) if isinstance(s.get('next_token'), list) else 0,
                    "tps": tps,
                    "kv": s.get('n_ctx', 0) / 1024.0 # in k
                })
                
            return {"slots": processed_slots, "queue": props.get('queue_size', 0)}
        except Exception:
            return None

class GraphWidget(PlotextPlot):
    def __init__(self, title: str, color: str = "cyan", y_min=None, y_max=None, **kwargs):
        super().__init__(**kwargs)
        self._title = title
        self._color = color
        self._y_min = y_min
        self._y_max = y_max
        self.data = deque(maxlen=MAX_BUFFER)

    def update_data(self, value: float):
        self.data.append(value)
        self.refresh_chart()

    def refresh_chart(self):
        plt = self.plt
        plt.clear_data()
        plt.clear_color()
        plt.theme("dark")
        
        if len(self.data) > 0:
            plt.plot(list(self.data), color=self._color)
            
        plt.title(self._title)
        if self._y_min is not None and self._y_max is not None:
            plt.ylim(self._y_min, self._y_max)
        
        plt.xticks([]) # Hide X axis
        self.refresh()

class SlotDetail(Static):
    def __init__(self, slot_id: int, **kwargs):
        super().__init__(**kwargs)
        self.slot_id = slot_id
        self.tps_graph = GraphWidget(f"Slot {slot_id} TPS", color="yellow")
        self.kv_graph = GraphWidget(f"Slot {slot_id} KV Cache", color="green")

    def compose(self) -> ComposeResult:
        with Container(classes="slot-card"):
            yield Static(f"[bold cyan]Slot {self.slot_id} Metrics", classes="metric-title")
            with Horizontal():
                yield self.tps_graph
                yield self.kv_graph

    def update_metrics(self, slot_data: Dict[str, Any]):
        self.tps_graph.update_data(slot_data['tps'])
        self.kv_graph.update_data(slot_data['kv'])

class TarsMonitor(App):
    CSS_PATH = "monitor.tcss"
    BINDINGS = [("c", "clear_logs", "Clear Logs"), ("q", "quit", "Quit")]

    def __init__(self):
        super().__init__()
        self.engine = TelemetryEngine()
        
        # State Buffers
        self.gpu_load_plot = GraphWidget("GPU Compute Load %", color="cyan", y_min=0, y_max=105)
        self.mem_plot = GraphWidget("Unified Memory (GB)", color="magenta")
        self.power_plot = GraphWidget("Power Draw (W)", color="green")
        self.temp_plot = GraphWidget("Core Temp (C)", color="red", y_min=30, y_max=100)
        
        self.slot_widgets = {} # id -> SlotDetail

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        with Horizontal():
            with Vertical(id="sidebar"):
                with Container(classes="widget-container"):
                    yield Static("MEMORY: 0.0 / 0.0 GB", id="mem-text")
                    yield self.mem_plot
                with Container(classes="widget-container"):
                    yield Static("GPU LOAD: 0%", id="gpu-text")
                    yield self.gpu_load_plot
                with Horizontal():
                    with Container(classes="widget-container half-width"):
                        yield self.power_plot
                    with Container(classes="widget-container half-width"):
                        yield self.temp_plot
            
            with Vertical(id="main-content"):
                yield DataTable(id="slots-table")
                with Vertical(id="active-slots"):
                    # Dynamic slots will be added here
                    pass
                yield RichLog(id="logs", wrap=True, highlight=True, markup=True)
        yield Footer()

    def on_mount(self) -> None:
        table = self.query_one("#slots-table", DataTable)
        table.add_columns("ID", "Status", "Task", "Progress", "TPS", "KV")
        self.set_interval(1 / REFRESH_RATE, self.update_telemetry)
        self.log_info("Tars Inference Monitor v1.2 Protocol Initiated.")

    def log_info(self, msg: str):
        self.query_one("#logs", RichLog).write(f"[dim]{time.strftime('%H:%M:%S')}[/dim] [cyan]INFO[/cyan] {msg}")

    def log_warn(self, msg: str):
        self.query_one("#logs", RichLog).write(f"[dim]{time.strftime('%H:%M:%S')}[/dim] [yellow]WARN[/yellow] {msg}")

    async def update_telemetry(self) -> None:
        # GPU Update
        gpu = self.engine.get_gpu_data()
        if gpu:
            self.query_one("#mem-text", Static).update(f"MEMORY: [bold]{gpu['mem_used']:.1f}[/bold] / {gpu['mem_total']:.1f} GB")
            self.query_one("#gpu-text", Static).update(f"GPU LOAD: [bold]{int(gpu['usage'])}%[/bold]")
            self.gpu_load_plot.update_data(gpu['usage'])
            self.mem_plot.update_data(gpu['mem_used'])
            self.power_plot.update_data(gpu['power'])
            self.temp_plot.update_data(gpu['temp'])
        else:
            self.log_warn("Failed to poll ROCm telemetry.")

        # Llama Update
        llama = self.engine.get_inference_data()
        table = self.query_one("#slots-table", DataTable)
        table.clear()
        
        if llama:
            for s in llama['slots']:
                status = "[bold green]BUSY[/bold green]" if s['busy'] else "[dim]IDLE[/dim]"
                prog = f"{s['decoded']}/{s['decoded']+s['remain']}"
                table.add_row(str(s['id']), status, str(s['task'])[-6:], prog, f"{s['tps']:.1f}", f"{s['kv']:.1f}k")
                
                # Dynamic Slot Graphs
                if s['busy']:
                    if s['id'] not in self.slot_widgets:
                        new_slot = SlotDetail(s['id'])
                        self.slot_widgets[s['id']] = new_slot
                        await self.query_one("#active-slots").mount(new_slot)
                    self.slot_widgets[s['id']].update_metrics(s)
                else:
                    if s['id'] in self.slot_widgets:
                        # Optionally keep or remove. Spec says "only render if busy"
                        # For now we'll just dim them or we could unmount
                        pass
        else:
            self.log_warn("Llama server unreachable on 8086.")

    def action_clear_logs(self):
        self.query_one("#logs", RichLog).clear()

if __name__ == "__main__":
    TarsMonitor().run()
