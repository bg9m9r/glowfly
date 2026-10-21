// Win32 helpers for scripts/smoke-windows.ps1: finding and placing Glowfly's window, bringing a
// window to the front, and real mouse and keyboard input. Every input goes through SendInput,
// the same path as hardware input; nothing here posts or sends window messages. No Alt key is
// ever pressed: a lone Alt tap opens Glowfly's title bar menu on Windows.
//
// Every method that reads or sets coordinates first makes its thread per-monitor DPI aware, so
// GetClientRect, ClientToScreen, SetWindowPos and the cursor all work in physical pixels.

using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace GlowflySmoke
{
    public static class Native
    {
        [StructLayout(LayoutKind.Sequential)]
        struct MOUSEINPUT { public int dx; public int dy; public uint mouseData; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

        [StructLayout(LayoutKind.Sequential)]
        struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }

        [StructLayout(LayoutKind.Sequential)]
        struct HARDWAREINPUT { public uint uMsg; public ushort wParamL; public ushort wParamH; }

        // MOUSEINPUT is the largest member, so INPUT is 40 bytes on x64 as SendInput expects.
        [StructLayout(LayoutKind.Explicit)]
        struct InputUnion
        {
            [FieldOffset(0)] public MOUSEINPUT mi;
            [FieldOffset(0)] public KEYBDINPUT ki;
            [FieldOffset(0)] public HARDWAREINPUT hi;
        }

        [StructLayout(LayoutKind.Sequential)]
        struct INPUT { public uint type; public InputUnion u; }

        [StructLayout(LayoutKind.Sequential)]
        struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

        [StructLayout(LayoutKind.Sequential)]
        struct POINT { public int X; public int Y; }

        [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
        struct DEVMODE
        {
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
            public ushort dmSpecVersion; public ushort dmDriverVersion; public ushort dmSize; public ushort dmDriverExtra;
            public uint dmFields;
            public int dmPositionX; public int dmPositionY; public uint dmDisplayOrientation; public uint dmDisplayFixedOutput;
            public short dmColor; public short dmDuplex; public short dmYResolution; public short dmTTOption; public short dmCollate;
            [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
            public ushort dmLogPixels; public uint dmBitsPerPel; public uint dmPelsWidth; public uint dmPelsHeight;
            public uint dmDisplayFlags; public uint dmDisplayFrequency;
            public uint dmICMMethod; public uint dmICMIntent; public uint dmMediaType; public uint dmDitherType;
            public uint dmReserved1; public uint dmReserved2; public uint dmPanningWidth; public uint dmPanningHeight;
        }

        delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr lParam);

        [DllImport("user32.dll", SetLastError = true)] static extern uint SendInput(uint count, INPUT[] inputs, int size);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SetCursorPos(int x, int y);
        [DllImport("user32.dll", SetLastError = true)] static extern bool GetCursorPos(out POINT p);
        [DllImport("user32.dll")] static extern int GetSystemMetrics(int index);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SystemParametersInfo(uint action, uint param, ref RECT rect, uint winIni);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SystemParametersInfo(uint action, uint param, ref uint value, uint winIni);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SystemParametersInfo(uint action, uint param, IntPtr value, uint winIni);
        [DllImport("user32.dll")] static extern bool EnumWindows(EnumWindowsProc proc, IntPtr lParam);
        [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
        [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int max);
        [DllImport("user32.dll")] static extern bool IsIconic(IntPtr hwnd);
        [DllImport("user32.dll")] static extern IntPtr GetWindow(IntPtr hwnd, uint cmd);
        [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr hwnd);
        [DllImport("user32.dll")] static extern bool BringWindowToTop(IntPtr hwnd);
        [DllImport("user32.dll")] static extern bool AttachThreadInput(uint attach, uint to, bool on);
        [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
        [DllImport("user32.dll")] static extern bool ShowWindow(IntPtr hwnd, int cmd);
        [DllImport("user32.dll", SetLastError = true)] static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int w, int h, uint flags);
        [DllImport("user32.dll", SetLastError = true)] static extern bool GetClientRect(IntPtr hwnd, out RECT rect);
        [DllImport("user32.dll")] static extern bool ClientToScreen(IntPtr hwnd, ref POINT p);
        [DllImport("user32.dll")] static extern uint GetDpiForWindow(IntPtr hwnd);
        [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
        [DllImport("user32.dll")] static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);
        [DllImport("user32.dll")] static extern uint MapVirtualKeyW(uint code, uint mapType);
        [DllImport("user32.dll")] static extern short VkKeyScanW(char ch);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool EnumDisplaySettingsW(string device, int mode, ref DEVMODE dm);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsExW(string device, ref DEVMODE dm, IntPtr hwnd, uint flags, IntPtr lParam);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettingsExW(string device, IntPtr dm, IntPtr hwnd, uint flags, IntPtr lParam);

        const uint INPUT_MOUSE = 0, INPUT_KEYBOARD = 1;
        const uint MOUSEEVENTF_MOVE = 0x1, MOUSEEVENTF_LEFTDOWN = 0x2, MOUSEEVENTF_LEFTUP = 0x4;
        const uint MOUSEEVENTF_RIGHTDOWN = 0x8, MOUSEEVENTF_RIGHTUP = 0x10;
        const uint MOUSEEVENTF_VIRTUALDESK = 0x4000, MOUSEEVENTF_ABSOLUTE = 0x8000;
        const uint KEYEVENTF_KEYUP = 0x2;
        const ushort VK_SHIFT = 0x10;
        const int SM_XVIRTUALSCREEN = 76, SM_YVIRTUALSCREEN = 77, SM_CXVIRTUALSCREEN = 78, SM_CYVIRTUALSCREEN = 79;
        const uint SPI_GETWORKAREA = 0x30, SPI_GETFOREGROUNDLOCKTIMEOUT = 0x2000, SPI_SETFOREGROUNDLOCKTIMEOUT = 0x2001;
        const uint GW_OWNER = 4;
        const int SW_RESTORE = 9;
        const uint SWP_NOZORDER = 0x4, SWP_NOACTIVATE = 0x10;
        const uint PW_RENDERFULLCONTENT = 2;
        const uint MAPVK_VK_TO_VSC = 0;
        const int ENUM_CURRENT_SETTINGS = -1;
        const uint DM_PELSWIDTH = 0x80000, DM_PELSHEIGHT = 0x100000;
        static readonly IntPtr DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 = new IntPtr(-4);

        static bool screenChanged;

        static void PerMonitorDpi() { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2); }

        /// The size SendInput expects for one INPUT (40 on x64), checked by the dry run.
        public static int InputSize() { return Marshal.SizeOf(typeof(INPUT)); }

        static void Send(params INPUT[] inputs)
        {
            uint sent = SendInput((uint)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));
            if (sent < inputs.Length) throw new Win32Exception();
        }

        static INPUT Mouse(uint flags, int dx, int dy)
        {
            var i = new INPUT { type = INPUT_MOUSE };
            i.u.mi = new MOUSEINPUT { dx = dx, dy = dy, dwFlags = flags };
            return i;
        }

        /// The largest visible, unowned top-level window of process `pid`, or Zero.
        public static IntPtr FindMainWindow(int pid)
        {
            PerMonitorDpi();
            IntPtr best = IntPtr.Zero;
            long bestArea = -1;
            EnumWindows((hwnd, _) =>
            {
                uint owner;
                GetWindowThreadProcessId(hwnd, out owner);
                if (owner != (uint)pid || !IsWindowVisible(hwnd) || GetWindow(hwnd, GW_OWNER) != IntPtr.Zero) return true;
                RECT r;
                if (!GetClientRect(hwnd, out r)) return true;
                long area = (long)(r.Right - r.Left) * (r.Bottom - r.Top);
                if (area > bestArea) { best = hwnd; bestArea = area; }
                return true;
            }, IntPtr.Zero);
            return best;
        }

        /// Makes `hwnd` the foreground window: with the foreground thread's input attached, so
        /// the system lets a background process do it. True when it is the foreground after.
        public static bool BringToFront(IntPtr hwnd)
        {
            PerMonitorDpi();
            if (IsIconic(hwnd)) ShowWindow(hwnd, SW_RESTORE);
            IntPtr fg = GetForegroundWindow();
            uint me = GetCurrentThreadId();
            uint other = 0;
            if (fg != IntPtr.Zero) { uint ignored; other = GetWindowThreadProcessId(fg, out ignored); }
            bool attached = other != 0 && other != me && AttachThreadInput(me, other, true);
            try
            {
                SetForegroundWindow(hwnd);
                BringWindowToTop(hwnd);
            }
            finally
            {
                if (attached) AttachThreadInput(me, other, false);
            }
            return GetForegroundWindow() == hwnd;
        }

        public static IntPtr Foreground() { return GetForegroundWindow(); }

        /// The window's title.
        public static string Title(IntPtr hwnd)
        {
            var text = new StringBuilder(256);
            GetWindowTextW(hwnd, text, text.Capacity);
            return text.ToString();
        }

        /// The client area on screen, in physical pixels: [x, y, w, h].
        public static int[] ClientOnScreen(IntPtr hwnd)
        {
            PerMonitorDpi();
            RECT r;
            if (!GetClientRect(hwnd, out r)) throw new Win32Exception();
            var p = new POINT();
            ClientToScreen(hwnd, ref p);
            return new[] { p.X, p.Y, r.Right - r.Left, r.Bottom - r.Top };
        }

        /// Restores `hwnd` and moves it to (x, y) at w x h, without activating it.
        public static void Place(IntPtr hwnd, int x, int y, int w, int h)
        {
            PerMonitorDpi();
            ShowWindow(hwnd, SW_RESTORE);
            if (!SetWindowPos(hwnd, IntPtr.Zero, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE)) throw new Win32Exception();
        }

        public static uint Dpi(IntPtr hwnd) { return GetDpiForWindow(hwnd); }

        /// The primary monitor's work area (without the taskbar): [x, y, w, h].
        public static int[] WorkArea()
        {
            PerMonitorDpi();
            var r = new RECT();
            if (!SystemParametersInfo(SPI_GETWORKAREA, 0, ref r, 0)) throw new Win32Exception();
            return new[] { r.Left, r.Top, r.Right - r.Left, r.Bottom - r.Top };
        }

        /// The rectangle around every monitor: [x, y, w, h].
        public static int[] VirtualScreen()
        {
            PerMonitorDpi();
            return new[] { GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN),
                GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        }

        /// Moves the pointer to screen pixel (x, y) with an absolute move; the normalised
        /// coordinates can round a pixel off, which the cursor position then corrects.
        public static void MoveTo(int x, int y)
        {
            PerMonitorDpi();
            int[] v = VirtualScreen();
            int nx = (int)(((long)(x - v[0]) * 65535 + (v[2] - 1) / 2) / Math.Max(1, v[2] - 1));
            int ny = (int)(((long)(y - v[1]) * 65535 + (v[3] - 1) / 2) / Math.Max(1, v[3] - 1));
            Send(Mouse(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK, nx, ny));
            POINT p;
            if (GetCursorPos(out p) && (p.X != x || p.Y != y)) SetCursorPos(x, y);
        }

        /// Presses (`down`) or releases a mouse button: 1 is left, 3 is right.
        public static void Button(int button, bool down)
        {
            PerMonitorDpi();
            uint flags;
            if (button == 1) flags = down ? MOUSEEVENTF_LEFTDOWN : MOUSEEVENTF_LEFTUP;
            else if (button == 3) flags = down ? MOUSEEVENTF_RIGHTDOWN : MOUSEEVENTF_RIGHTUP;
            else throw new ArgumentException("button must be 1 (left) or 3 (right), not " + button);
            Send(Mouse(flags, 0, 0));
        }

        /// Presses or releases virtual key `vk`, with its scan code so the physical key arrives too.
        public static void Key(ushort vk, bool down)
        {
            PerMonitorDpi();
            if (vk == 0x12 || vk == 0xA4 || vk == 0xA5) throw new ArgumentException("Alt is never sent: it opens the title bar menu");
            var i = new INPUT { type = INPUT_KEYBOARD };
            i.u.ki = new KEYBDINPUT { wVk = vk, wScan = (ushort)MapVirtualKeyW(vk, MAPVK_VK_TO_VSC), dwFlags = down ? 0 : KEYEVENTF_KEYUP };
            Send(i);
        }

        /// Presses the keys in order, then releases them in reverse order.
        public static void Chord(params ushort[] vks)
        {
            foreach (ushort vk in vks) { Key(vk, true); Thread.Sleep(30); }
            for (int n = vks.Length - 1; n >= 0; n--) { Key(vks[n], false); Thread.Sleep(30); }
        }

        /// Types `s` as key presses of the current layout, `delayMs` apart. Throws before sending
        /// anything when a character needs more than Shift or the layout has no key for it.
        public static void Text(string s, int delayMs)
        {
            var keys = new short[s.Length];
            for (int n = 0; n < s.Length; n++)
            {
                short k = VkKeyScanW(s[n]);
                if (k == -1) throw new ArgumentException("the keyboard layout has no key for '" + s[n] + "'");
                if (((k >> 8) & ~1) != 0) throw new ArgumentException("'" + s[n] + "' needs Ctrl or Alt on this layout");
                keys[n] = k;
            }
            foreach (short k in keys)
            {
                bool shift = ((k >> 8) & 1) != 0;
                ushort vk = (ushort)(k & 0xFF);
                if (shift) Key(VK_SHIFT, true);
                Key(vk, true);
                Key(vk, false);
                if (shift) Key(VK_SHIFT, false);
                Thread.Sleep(delayMs);
            }
        }

        /// Draws `hwnd` into `hdc` as composed, even while it is covered.
        public static bool PrintTo(IntPtr hwnd, IntPtr hdc)
        {
            PerMonitorDpi();
            return PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT);
        }

        /// Switches the display, for this session only, to its largest mode up to maxW x maxH.
        /// False (and nothing changed) when no mode of at least minW x minH fits.
        public static bool SetScreenSize(int maxW, int maxH, int minW, int minH)
        {
            PerMonitorDpi();
            var current = NewDevMode();
            if (!EnumDisplaySettingsW(null, ENUM_CURRENT_SETTINGS, ref current)) return false;
            DEVMODE best = current;
            long bestArea = -1;
            var dm = NewDevMode();
            for (int mode = 0; EnumDisplaySettingsW(null, mode, ref dm); mode++)
            {
                long area = (long)dm.dmPelsWidth * dm.dmPelsHeight;
                if (dm.dmPelsWidth <= maxW && dm.dmPelsHeight <= maxH && dm.dmBitsPerPel == current.dmBitsPerPel && area > bestArea)
                {
                    best = dm;
                    bestArea = area;
                }
                dm = NewDevMode();
            }
            if (bestArea < 0 || best.dmPelsWidth < minW || best.dmPelsHeight < minH) return false;
            if (best.dmPelsWidth == current.dmPelsWidth && best.dmPelsHeight == current.dmPelsHeight) return true;
            best.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;
            if (ChangeDisplaySettingsExW(null, ref best, IntPtr.Zero, 0, IntPtr.Zero) != 0) return false;
            screenChanged = true;
            return true;
        }

        /// Puts back the display mode from before SetScreenSize.
        public static void RestoreScreen()
        {
            if (!screenChanged) return;
            ChangeDisplaySettingsExW(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
            screenChanged = false;
        }

        static DEVMODE NewDevMode()
        {
            var dm = new DEVMODE { dmDeviceName = "", dmFormName = "" };
            dm.dmSize = (ushort)Marshal.SizeOf(typeof(DEVMODE));
            return dm;
        }

        /// How long, in ms, after user input the system refuses SetForegroundWindow to others.
        public static uint ForegroundLockTimeout
        {
            get
            {
                uint value = 0;
                if (!SystemParametersInfo(SPI_GETFOREGROUNDLOCKTIMEOUT, 0, ref value, 0)) throw new Win32Exception();
                return value;
            }
            set
            {
                if (!SystemParametersInfo(SPI_SETFOREGROUNDLOCKTIMEOUT, 0, new IntPtr(value), 0)) throw new Win32Exception();
            }
        }
    }
}
