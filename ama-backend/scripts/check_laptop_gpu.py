"""Run with the laptop's ML Python. Optional --request benchmarks a worker job.

No downloads or inference unless --request is supplied. Reports Torch memory,
not total process/driver/display VRAM. Run one model at a time.
"""
import argparse
import json
from pathlib import Path
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--request', type=Path)
    parser.add_argument('--report', type=Path, default=Path('laptop-gpu-report.json'))
    args = parser.parse_args()
    report = {'inference_tested': False}
    try:
        import torch
        report.update(torch_version=torch.__version__, cuda_runtime=torch.version.cuda,
                      cuda_available=torch.cuda.is_available())
        if not torch.cuda.is_available():
            raise RuntimeError('CUDA unavailable in this Python runtime; check driver and Torch installation')
        device = torch.cuda.get_device_properties(0)
        free, total = torch.cuda.mem_get_info(0)
        report.update(gpu=device.name, total_vram_gib=total / 2**30,
                      free_vram_gib=free / 2**30, bf16_supported=torch.cuda.is_bf16_supported())
        if args.request:
            request = json.loads(args.request.read_text(encoding='utf-8'))
            if not str(request['config'].get('device', '')).startswith('cuda'):
                raise ValueError('Benchmark request must explicitly select a CUDA device')
            sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'backend'))
            from betun.model_worker import main as run_model
            torch.cuda.reset_peak_memory_stats()
            start = time.perf_counter()
            run_model(str(args.request), str(args.report.with_suffix('.result.json')))
            torch.cuda.synchronize()
            report.update(inference_tested=True, elapsed_seconds=time.perf_counter()-start,
                          peak_allocated_gib=torch.cuda.max_memory_allocated()/2**30,
                          peak_reserved_gib=torch.cuda.max_memory_reserved()/2**30,
                          provider=request['config'].get('provider'))
    except Exception as error:
        report['error'] = str(error)
    args.report.write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2))
    return 1 if 'error' in report else 0


if __name__ == '__main__':
    raise SystemExit(main())
