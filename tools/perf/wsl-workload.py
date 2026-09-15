#!/usr/bin/env python3
"""Controlled WSL workloads; GUI Lua round-trip fences, NOT GPU presentation."""
import argparse
import base64
import json
import os
from pathlib import Path
import select
import struct
import termios
import time
import tty
import zlib


def publish(path, value):
    path = Path(path)
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value), encoding='utf-8')
    temporary.replace(path)


def write_all(data):
    view = memoryview(data)
    while view:
        count = os.write(1, view)
        if count <= 0:
            raise RuntimeError('terminal output closed')
        view = view[count:]


_fence_sequence = 0


def fence(timeout=30, startup=False):
    global _fence_sequence
    # DSR is insufficient: ConPTY may answer it before the GUI parses output.
    # During startup the native window may not yet subscribe to pane events,
    # so retry with fresh tokens until its Lua callback becomes available.
    deadline = time.monotonic() + timeout
    response = b''
    acknowledgments = []
    next_send = 0
    while time.monotonic() < deadline:
        if time.monotonic() >= next_send:
            _fence_sequence += 1
            token = f'{os.getpid()}-{_fence_sequence}'.encode()
            write_all(b'\x1b]1337;SetUserVar=PERF_FENCE=' + base64.b64encode(token) + b'\x07')
            acknowledgments.append(b'PERF_ACK:' + token + b':END')
            next_send = time.monotonic() + 0.02 if startup else deadline
        wait = max(0, min(next_send, deadline) - time.monotonic())
        if select.select([0], [], [], wait)[0]:
            data = os.read(0, 4096)
            if not data:
                raise RuntimeError('terminal input closed')
            response += data
            if any(ack in response for ack in acknowledgments):
                return
    raise TimeoutError(f'GUI Lua fence not acknowledged; input tail={response[-200:]!r}')


def png(seed):
    def chunk(kind, body):
        return (struct.pack('>I', len(body)) + kind + body
                + struct.pack('>I', zlib.crc32(kind + body)))
    width = height = 256
    row = bytes((seed % 256, (seed * 7) % 256, (seed * 13) % 256, 255)) * width
    raw = (b'\0' + row) * height
    return (b'\x89PNG\r\n\x1a\n'
            + chunk(b'IHDR', struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b''))


def verify_kitty():
    # Untimed capability check: reject runs where ConPTY drops the APC or the
    # terminal does not decode PNGs. This does not prove GPU presentation.
    write_all(b'\x1b_Ga=q,f=100,i=9001;' + base64.b64encode(png(17)) + b'\x1b\\')
    response = b''
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if select.select([0], [], [], max(0, deadline - time.monotonic()))[0]:
            data = os.read(0, 4096)
            if not data:
                break
            response += data
            if b'\x1b_Gi=9001;OK\x1b\\' in response:
                return
    raise RuntimeError(f'Kitty PNG query not acknowledged: {response[-200:]!r}')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--result', required=True)
    parser.add_argument('--mode', choices=['idle', 'ascii', 'ansi', 'unicode', 'kitty'], required=True)
    parser.add_argument('--mib', type=int, default=32)
    parser.add_argument('--idle-seconds', type=float, default=5)
    args = parser.parse_args()
    ready = args.result + '.ready'
    go = Path(args.result + '.go')
    original = termios.tcgetattr(0)
    try:
        tty.setraw(0)
        termios.tcflush(0, termios.TCIFLUSH)
        fence(startup=True)
        if args.mode == 'kitty':
            verify_kitty()
        publish(ready, {'ready': True, 'pid': os.getpid()})
        deadline = time.monotonic() + 60
        while not go.exists():
            if time.monotonic() > deadline:
                raise TimeoutError('controller did not start measurement')
            time.sleep(0.01)
        lines = {
            'ascii': b'0123456789 abcdefghijklmnopqrstuvwxyz ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789\r\n',
            'ansi': b'\x1b[31mERROR\x1b[0m \x1b[1;32mSUCCESS\x1b[0m value=1234567890 lorem ipsum dolor sit amet\r\n',
            'unicode': ('中文输入与终端渲染测试 日本語 한국어 café λ → 0123456789\r\n').encode(),
        }
        # Prepare data before the measured interval.
        payloads = []
        if args.mode in lines:
            line = lines[args.mode]
            payloads = [line * max(1, 65536 // len(line))]
        elif args.mode == 'kitty':
            # Unique image contents, same ID: bounded storage, no cache-only benchmark.
            payloads = [b'\x1b[H\x1b_Ga=T,f=100,q=2,i=1,c=32,r=16;'
                        + base64.b64encode(png(i)) + b'\x1b\\' for i in range(120)]
        started = time.perf_counter()
        byte_count = 0
        fences = 0
        if args.mode == 'idle':
            time.sleep(args.idle_seconds)
        elif args.mode == 'kitty':
            for payload in payloads:
                write_all(payload)
                byte_count += len(payload)
        else:
            payload = payloads[0]
            while byte_count < args.mib * 1024 * 1024:
                write_all(payload)
                byte_count += len(payload)
        fence()
        elapsed = time.perf_counter() - started
        publish(args.result, {
            'mode': args.mode, 'bytes': byte_count, 'seconds': elapsed,
            'mib_per_second': byte_count / (1024 * 1024) / elapsed,
            'images': len(payloads) if args.mode == 'kitty' else 0,
            'image_fences': fences,
            'kitty_query_verified': args.mode == 'kitty',
            'scope': 'WSL producer + ConPTY + GUI parser/Lua acknowledgment; not a GPU present fence',
        })
        # Allow the controller to sample final live-process memory/CPU before exit.
        deadline = time.monotonic() + 60
        while not Path(args.result + '.done').exists() and time.monotonic() < deadline:
            time.sleep(0.01)
    except Exception as error:
        publish(args.result, {'error': str(error)})
        raise
    finally:
        termios.tcsetattr(0, termios.TCSANOW, original)


if __name__ == '__main__':
    main()
