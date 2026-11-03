// GUARDIANA, «ligero» (1.0.55): lo que cuesta en un Windows de verdad cada cosa que el servicio hace solo.
// Arranca una orden dentro de un objeto de trabajo de Windows (suspendida, metida en el trabajo y luego en marcha) y
// cuenta lo que gastaron ella y todo lo que arrancó (conhost incluido): tiempo de procesador, procesos, memoria y
// lectura de disco. Aparte, lo que gastó el equipo entero mientras tanto (GetSystemTimes), para ver también el trabajo
// que una orden deja a otros procesos (WmiPrvSE, Defender). Solo se usa en .github/workflows/ligero.yml.
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;

public static class Medidor
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct STARTUPINFO
    {
        public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
        public int dwX, dwY, dwXSize, dwYSize, dwXCountChars, dwYCountChars, dwFillAttribute, dwFlags;
        public short wShowWindow, cbReserved2; public IntPtr lpReserved2, hStdInput, hStdOutput, hStdError;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct PROCESS_INFORMATION { public IntPtr hProcess, hThread; public int dwProcessId, dwThreadId; }

    [StructLayout(LayoutKind.Sequential)]
    struct BASIC
    {
        public long TotalUserTime, TotalKernelTime, ThisPeriodTotalUserTime, ThisPeriodTotalKernelTime;
        public uint TotalPageFaultCount, TotalProcesses, ActiveProcesses, TotalTerminatedProcesses;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct IO_COUNTERS
    {
        public ulong ReadOperationCount, WriteOperationCount, OtherOperationCount;
        public ulong ReadTransferCount, WriteTransferCount, OtherTransferCount;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct BASIC_AND_IO { public BASIC Basic; public IO_COUNTERS Io; }

    [StructLayout(LayoutKind.Sequential)]
    struct BASIC_LIMIT
    {
        public long PerProcessUserTimeLimit, PerJobUserTimeLimit; public uint LimitFlags;
        public UIntPtr MinimumWorkingSetSize, MaximumWorkingSetSize; public uint ActiveProcessLimit;
        public UIntPtr Affinity; public uint PriorityClass, SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct EXTENDED_LIMIT
    {
        public BASIC_LIMIT Basic; public IO_COUNTERS Io;
        public UIntPtr ProcessMemoryLimit, JobMemoryLimit, PeakProcessMemoryUsed, PeakJobMemoryUsed;
    }

    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr CreateJobObject(IntPtr atributos, string nombre);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool AssignProcessToJobObject(IntPtr trabajo, IntPtr proceso);
    [DllImport("kernel32.dll", SetLastError = true, EntryPoint = "QueryInformationJobObject")]
    static extern bool Cuentas(IntPtr trabajo, int clase, out BASIC_AND_IO info, int largo, IntPtr devuelto);
    [DllImport("kernel32.dll", SetLastError = true, EntryPoint = "QueryInformationJobObject")]
    static extern bool Limites(IntPtr trabajo, int clase, out EXTENDED_LIMIT info, int largo, IntPtr devuelto);
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool CreateProcess(string app, StringBuilder linea, IntPtr pa, IntPtr ta, bool hereda, uint banderas,
        IntPtr entorno, string carpeta, ref STARTUPINFO si, out PROCESS_INFORMATION pi);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern uint ResumeThread(IntPtr hilo);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern uint WaitForSingleObject(IntPtr asa, uint ms);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool CloseHandle(IntPtr asa);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetExitCodeProcess(IntPtr asa, out uint codigo);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetSystemTimes(out long reposo, out long nucleo, out long usuario);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern IntPtr OpenProcess(uint acceso, bool hereda, int pid);

    [StructLayout(LayoutKind.Sequential)]
    struct PRIVILEGIO { public uint Cuantos; public uint LuidBajo; public int LuidAlto; public uint Atributos; }
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool OpenProcessToken(IntPtr proceso, uint acceso, out IntPtr ficha);
    [DllImport("advapi32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool LookupPrivilegeValue(string sistema, string nombre, out long luid);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool AdjustTokenPrivileges(IntPtr ficha, bool quitarTodos, ref PRIVILEGIO nuevo, int largo, IntPtr previo, IntPtr devuelto);
    [DllImport("kernel32.dll")]
    static extern IntPtr GetCurrentProcess();

    /// <summary>
    /// Enciende SeDebugPrivilege en esta sesión de PowerShell (el administrador lo tiene, apagado): hace falta para
    /// meter en un trabajo el proceso del servicio, que es de SYSTEM.
    /// </summary>
    public static void Depurar()
    {
        IntPtr ficha;
        if (!OpenProcessToken(GetCurrentProcess(), 0x20 | 0x8, out ficha)) throw new Win32Exception();
        try
        {
            long luid;
            if (!LookupPrivilegeValue(null, "SeDebugPrivilege", out luid)) throw new Win32Exception();
            PRIVILEGIO p = new PRIVILEGIO();
            p.Cuantos = 1; p.LuidBajo = (uint)(luid & 0xffffffff); p.LuidAlto = (int)(luid >> 32); p.Atributos = 2;
            if (!AdjustTokenPrivileges(ficha, false, ref p, 0, IntPtr.Zero, IntPtr.Zero)) throw new Win32Exception();
            int e = Marshal.GetLastWin32Error();
            if (e != 0) throw new Win32Exception(e);
        }
        finally { CloseHandle(ficha); }
    }

    const uint CREATE_SUSPENDED = 0x4;
    const uint CREATE_NO_WINDOW = 0x08000000;

    /// <summary>Lo que el equipo entero lleva gastado (núcleo sin reposo, más usuario), en ms.</summary>
    public static double EquipoMs()
    {
        long reposo, nucleo, usuario;
        if (!GetSystemTimes(out reposo, out nucleo, out usuario)) throw new Win32Exception();
        return (nucleo - reposo + usuario) / 10000.0;
    }

    /// <summary>Un trabajo nuevo, vacío.</summary>
    public static IntPtr Trabajo()
    {
        IntPtr t = CreateJobObject(IntPtr.Zero, null);
        if (t == IntPtr.Zero) throw new Win32Exception();
        return t;
    }

    /// <summary>Mete en el trabajo un proceso que ya está en marcha (el servicio, para contarlo durante minutos).</summary>
    public static void Mete(IntPtr trabajo, int pid)
    {
        const uint PROCESS_SET_QUOTA = 0x0100, PROCESS_TERMINATE = 0x0001, PROCESS_QUERY_LIMITED = 0x1000;
        IntPtr p = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED, false, pid);
        if (p == IntPtr.Zero) throw new Win32Exception();
        try { if (!AssignProcessToJobObject(trabajo, p)) throw new Win32Exception(); }
        finally { CloseHandle(p); }
    }

    /// <summary>Lo que lleva el trabajo: ms de procesador, procesos que pasaron por él, bytes leídos, memoria pico (MB).</summary>
    public static double[] Lleva(IntPtr trabajo)
    {
        BASIC_AND_IO c;
        if (!Cuentas(trabajo, 8, out c, Marshal.SizeOf(typeof(BASIC_AND_IO)), IntPtr.Zero)) throw new Win32Exception();
        EXTENDED_LIMIT l;
        if (!Limites(trabajo, 9, out l, Marshal.SizeOf(typeof(EXTENDED_LIMIT)), IntPtr.Zero)) throw new Win32Exception();
        return new double[] {
            (c.Basic.TotalUserTime + c.Basic.TotalKernelTime) / 10000.0,
            c.Basic.TotalProcesses,
            c.Io.ReadTransferCount,
            l.PeakJobMemoryUsed.ToUInt64() / 1048576.0,
            c.Basic.ActiveProcesses,
        };
    }

    /// <summary>
    /// Arranca `linea` dentro de `trabajo` y la deja en marcha (el servicio, para contarlo durante minutos): todo lo
    /// que arranque después cae en el mismo trabajo. Devuelve su número de proceso.
    /// </summary>
    public static int Arranca(IntPtr trabajo, string linea)
    {
        STARTUPINFO si = new STARTUPINFO();
        si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
        PROCESS_INFORMATION pi;
        if (!CreateProcess(null, new StringBuilder(linea), IntPtr.Zero, IntPtr.Zero, false,
                CREATE_SUSPENDED | CREATE_NO_WINDOW, IntPtr.Zero, null, ref si, out pi))
            throw new Win32Exception();
        try
        {
            if (!AssignProcessToJobObject(trabajo, pi.hProcess)) throw new Win32Exception();
            ResumeThread(pi.hThread);
            return pi.dwProcessId;
        }
        finally
        {
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
        }
    }

    /// <summary>
    /// Arranca `linea` (con la prioridad de `prioridad`, 0 la normal) dentro de un trabajo nuevo, espera a que acabe ella
    /// y todo lo que arrancó, y devuelve: ms de reloj, ms de procesador del trabajo, procesos, bytes leídos, memoria
    /// pico del trabajo (MB), ms de procesador del equipo entero mientras tanto, y el código de salida.
    /// </summary>
    public static double[] Mide(string linea, uint prioridad)
    {
        IntPtr trabajo = Trabajo();
        STARTUPINFO si = new STARTUPINFO();
        si.cb = Marshal.SizeOf(typeof(STARTUPINFO));
        PROCESS_INFORMATION pi;
        double equipo0 = EquipoMs();
        var reloj = System.Diagnostics.Stopwatch.StartNew();
        if (!CreateProcess(null, new StringBuilder(linea), IntPtr.Zero, IntPtr.Zero, false,
                CREATE_SUSPENDED | CREATE_NO_WINDOW | prioridad, IntPtr.Zero, null, ref si, out pi))
            throw new Win32Exception();
        try
        {
            if (!AssignProcessToJobObject(trabajo, pi.hProcess)) throw new Win32Exception();
            ResumeThread(pi.hThread);
            WaitForSingleObject(pi.hProcess, 300000);
            // conhost y lo que arrancara: hasta que no quede nadie en el trabajo (como mucho 10 s más)
            for (int i = 0; i < 200 && Lleva(trabajo)[4] > 0; i++) System.Threading.Thread.Sleep(50);
            reloj.Stop();
            double equipo = EquipoMs() - equipo0;
            uint codigo;
            GetExitCodeProcess(pi.hProcess, out codigo);
            double[] t = Lleva(trabajo);
            return new double[] { reloj.Elapsed.TotalMilliseconds, t[0], t[1], t[2], t[3], equipo, codigo };
        }
        finally
        {
            CloseHandle(pi.hThread);
            CloseHandle(pi.hProcess);
            CloseHandle(trabajo);
        }
    }
}
