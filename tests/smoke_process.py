"""Bounded subprocess capture for smoke tests that start background services."""
import ctypes
import itertools
import json
import os
from pathlib import Path
import signal
import subprocess
import threading
import time


class WindowsJob:
    """Keep test descendants alive between commands, then terminate them together."""

    def __init__(self):
        self.handle = None
        if os.name != 'nt':
            return
        from ctypes import wintypes as w

        class BasicLimits(ctypes.Structure):
            _fields_ = [('process_time', ctypes.c_int64), ('job_time', ctypes.c_int64),
                        ('flags', w.DWORD), ('min_working_set', ctypes.c_size_t),
                        ('max_working_set', ctypes.c_size_t), ('active_processes', w.DWORD),
                        ('affinity', ctypes.c_size_t), ('priority', w.DWORD), ('scheduling', w.DWORD)]

        class ExtendedLimits(ctypes.Structure):
            _fields_ = [('basic', BasicLimits), ('io', ctypes.c_uint64 * 6),
                        ('process_memory', ctypes.c_size_t), ('job_memory', ctypes.c_size_t),
                        ('peak_process_memory', ctypes.c_size_t), ('peak_job_memory', ctypes.c_size_t)]

        class Accounting(ctypes.Structure):
            _fields_ = [('times', ctypes.c_int64 * 4), ('page_faults', w.DWORD),
                        ('total_processes', w.DWORD), ('active_processes', w.DWORD),
                        ('terminated_processes', w.DWORD)]

        self.accounting = Accounting

        self.api = ctypes.WinDLL('kernel32', use_last_error=True)
        self.api.CreateJobObjectW.argtypes = [ctypes.c_void_p, w.LPCWSTR]
        self.api.CreateJobObjectW.restype = w.HANDLE
        self.api.SetInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD]
        self.api.SetInformationJobObject.restype = w.BOOL
        self.api.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
        self.api.AssignProcessToJobObject.restype = w.BOOL
        self.api.TerminateJobObject.argtypes = [w.HANDLE, w.UINT]
        self.api.TerminateJobObject.restype = w.BOOL
        self.api.QueryInformationJobObject.argtypes = [w.HANDLE, ctypes.c_int, ctypes.c_void_p, w.DWORD, ctypes.c_void_p]
        self.api.QueryInformationJobObject.restype = w.BOOL
        self.api.CloseHandle.argtypes = [w.HANDLE]
        self.api.CloseHandle.restype = w.BOOL
        self.handle = self.api.CreateJobObjectW(None, None)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE; no breakaway.
        if not self.api.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            error = ctypes.WinError(ctypes.get_last_error())
            self.close()
            raise error

    def assign(self, process):
        if self.handle and not self.api.AssignProcessToJobObject(self.handle, int(process._handle)):
            # A very short command may already have exited before assignment.
            error = ctypes.WinError(ctypes.get_last_error())
            if process.poll() is None:
                raise error

    def close(self):
        if self.handle:
            try:
                self.api.TerminateJobObject(self.handle, 1)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    accounting = self.accounting()
                    if not self.api.QueryInformationJobObject(self.handle, 1, ctypes.byref(accounting), ctypes.sizeof(accounting), None):
                        break
                    if accounting.active_processes == 0:
                        break
                    time.sleep(.02)
            finally:
                self.api.CloseHandle(self.handle)
                self.handle = None


class SmokeCommands:
    def __init__(self, output):
        self.output = Path(output)
        self.output.mkdir(parents=True, exist_ok=True)
        self.job = WindowsJob()
        self.counter = itertools.count(1)
        self.lock = threading.Lock()

    def run(self, command, *, env=None, timeout=240):
        with self.lock:
            number = next(self.counter)
            prefix = self.output / f'command-{number:03d}'
            prefix.with_suffix('.json').write_text(
                json.dumps(command, ensure_ascii=False), encoding='utf-8')
            print(f'[{number:03d}] START {json.dumps(command)}', flush=True)
        started = time.monotonic()
        # Files avoid communicate() waiting forever for a descendant's inherited
        # pipe, including Python's Windows TimeoutExpired cleanup path.
        with prefix.with_suffix('.stdout').open('wb') as stdout, prefix.with_suffix('.stderr').open('wb') as stderr:
            process = subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL,
                                       stdout=stdout, stderr=stderr, close_fds=True,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0,
                                       start_new_session=os.name != 'nt')
            try:
                self.job.assign(process)
                process.wait(timeout=timeout)
            except BaseException:
                if os.name != 'nt':
                    try:
                        os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                else:
                    process.kill()
                process.wait(timeout=5)
                print(f'[{number:03d}] FAILED after {time.monotonic() - started:.1f}s; logs: {prefix}', flush=True)
                raise
        result = subprocess.CompletedProcess(command, process.returncode,
                    prefix.with_suffix('.stdout').read_text(encoding='utf-8', errors='replace'),
                    prefix.with_suffix('.stderr').read_text(encoding='utf-8', errors='replace'))
        print(f'[{number:03d}] DONE exit={result.returncode} after {time.monotonic() - started:.1f}s', flush=True)
        return result

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.job.close()
