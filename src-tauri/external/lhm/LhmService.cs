// Headless LibreHardwareMonitor sensor service.
// Outputs sensor data as JSON lines to stdout every second.
// Exits when stdin is closed (parent process died).

using System;
using System.Text;
using System.Threading;
using LibreHardwareMonitor.Hardware;

class LhmService
{
    static Computer computer;

    static void Main(string[] args)
    {
        try
        {
            computer = new Computer
            {
                IsCpuEnabled = true,
                IsGpuEnabled = true,
                IsMemoryEnabled = true,
                IsMotherboardEnabled = true,
                IsControllerEnabled = true,
                IsNetworkEnabled = true,
                IsStorageEnabled = true
            };

            computer.Open();
            Console.Error.WriteLine("LHM: Sensors initialized");

            var updateVisitor = new UpdateVisitor();

            // Dump all sensors once for debugging
            if (args.Length > 0 && args[0] == "--dump")
            {
                computer.Accept(updateVisitor);
                foreach (var hw in computer.Hardware)
                    DumpHardware(hw, "");
                return;
            }

            while (true)
            {
                computer.Accept(updateVisitor);
                OutputSensors();

                // Sleep 1 second between updates
                Thread.Sleep(1000);
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine("LHM Error: " + ex.Message);
        }
        finally
        {
            if (computer != null)
                computer.Close();
        }
    }

    static void OutputSensors()
    {
        var sb = new StringBuilder();
        sb.Append("{");

        bool firstField = true;
        // Track emitted keys to prevent duplicates (e.g. AMD Ryzen with admin
        // exposes both Tctl/Tdie and per-CCD temps, both matching cpu_temp)
        var emitted = new System.Collections.Generic.HashSet<string>();

        foreach (var hardware in computer.Hardware)
        {
            ProcessHardware(hardware, sb, ref firstField, emitted);
        }

        sb.Append("}");

        try
        {
            Console.WriteLine(sb.ToString());
            Console.Out.Flush();
        }
        catch
        {
            // Stdout closed — parent died, exit gracefully
            Environment.Exit(0);
        }
    }

    static void ProcessHardware(IHardware hardware, StringBuilder sb, ref bool firstField,
        System.Collections.Generic.HashSet<string> emitted)
    {
        var hwType = hardware.HardwareType;

        foreach (var sensor in hardware.Sensors)
        {
            if (sensor.Value == null) continue;

            string key = null;
            float val = sensor.Value.Value;

            if (hwType == HardwareType.Cpu)
            {
                if (sensor.SensorType == SensorType.Temperature)
                {
                    // Prefer Package/Tctl/Tdie, fall back to any CPU temp
                    string sn = sensor.Name;
                    if (sn.Contains("Package") || sn.Contains("Tctl") || sn.Contains("Tdie") ||
                        sn.Contains("CPU") || sn.Contains("Core (Tctl"))
                    {
                        key = "cpu_temp";
                    }
                }
                else if (sensor.SensorType == SensorType.Load && sensor.Name == "CPU Total")
                {
                    key = "cpu_usage";
                }
                else if (sensor.SensorType == SensorType.Clock && sensor.Name.Contains("Core #0"))
                {
                    key = "cpu_freq";
                }
            }
            else if (hwType == HardwareType.GpuNvidia || hwType == HardwareType.GpuAmd || hwType == HardwareType.GpuIntel)
            {
                if (sensor.SensorType == SensorType.Temperature && sensor.Name.Contains("GPU Core"))
                {
                    key = "gpu_temp";
                }
                else if (sensor.SensorType == SensorType.Load && sensor.Name == "GPU Core")
                {
                    key = "gpu_usage";
                }
                else if (sensor.SensorType == SensorType.Clock && sensor.Name.Contains("GPU Core"))
                {
                    key = "gpu_freq";
                }
                else if (sensor.SensorType == SensorType.SmallData && sensor.Name.Contains("GPU Memory Used"))
                {
                    key = "gpu_mem_used";
                }
                else if (sensor.SensorType == SensorType.SmallData && sensor.Name.Contains("GPU Memory Total"))
                {
                    key = "gpu_mem_total";
                }
            }

            // Skip NaN, zero, and duplicate keys
            if (key != null && !float.IsNaN(val) && val != 0 && !emitted.Contains(key))
            {
                emitted.Add(key);
                if (!firstField) sb.Append(",");
                firstField = false;
                sb.Append("\"");
                sb.Append(key);
                sb.Append("\":");
                sb.Append(val.ToString(System.Globalization.CultureInfo.InvariantCulture));
            }
        }

        foreach (var sub in hardware.SubHardware)
        {
            ProcessHardware(sub, sb, ref firstField, emitted);
        }
    }

    static void DumpHardware(IHardware hw, string indent)
    {
        Console.WriteLine("{0}[{1}] {2}", indent, hw.HardwareType, hw.Name);
        foreach (var s in hw.Sensors)
            Console.WriteLine("{0}  {1}: {2} = {3}", indent, s.SensorType, s.Name, s.Value);
        foreach (var sub in hw.SubHardware)
            DumpHardware(sub, indent + "  ");
    }
}

class UpdateVisitor : IVisitor
{
    public void VisitComputer(IComputer computer) { computer.Traverse(this); }
    public void VisitHardware(IHardware hardware) { hardware.Update(); foreach (var sub in hardware.SubHardware) sub.Accept(this); }
    public void VisitSensor(ISensor sensor) { }
    public void VisitParameter(IParameter parameter) { }
}
