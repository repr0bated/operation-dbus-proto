import { useState } from "react";
import {
  Search,
  Eye,
  Database,
  ChevronRight,
  CheckCircle2,
  Circle,
  Loader2,
  ArrowRight,
} from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";

// Mock introspectable targets
const MOCK_TARGETS = [
  { id: "dbus", label: "D-Bus Service", sources: [
    { id: "org.freedesktop.NetworkManager", label: "NetworkManager", path: "/" },
    { id: "org.freedesktop.systemd1", label: "systemd", path: "/" },
    { id: "org.freedesktop.PackageKit", label: "PackageKit", path: "/" },
    { id: "org.freedesktop.UPower", label: "UPower", path: "/" },
  ]},
  { id: "gcloud", label: "GCloud CLI", sources: [
    { id: "compute", label: "compute", path: "gcloud compute" },
    { id: "container", label: "container", path: "gcloud container" },
    { id: "iam", label: "iam", path: "gcloud iam" },
  ]},
  { id: "docker", label: "Docker", sources: [
    { id: "containers", label: "Containers", path: "/containers" },
    { id: "images", label: "Images", path: "/images" },
  ]},
  { id: "ldap", label: "LDAP", sources: [
    { id: "ou=users", label: "Users OU", path: "ou=users,dc=example" },
    { id: "ou=groups", label: "Groups OU", path: "ou=groups,dc=example" },
  ]},
];

// Mock introspection result
const MOCK_RESULT = {
  name: "org.freedesktop.NetworkManager",
  path: "/",
  interfaces: [
    {
      name: "org.freedesktop.NetworkManager",
      methods: [
        { name: "GetDevices", args: [], returns: "ao" },
        { name: "ActivateConnection", args: ["o", "o", "o"], returns: "o" },
        { name: "DeactivateConnection", args: ["o"], returns: "" },
        { name: "Enable", args: ["b"], returns: "" },
      ],
      properties: [
        { name: "Version", type: "s", access: "read" },
        { name: "State", type: "u", access: "read" },
        { name: "Connectivity", type: "u", access: "read" },
        { name: "WirelessEnabled", type: "b", access: "readwrite" },
        { name: "NetworkingEnabled", type: "b", access: "read" },
      ],
      signals: [
        { name: "DeviceAdded", args: ["o"] },
        { name: "DeviceRemoved", args: ["o"] },
        { name: "StateChanged", args: ["u"] },
      ],
    },
    {
      name: "org.freedesktop.DBus.Properties",
      methods: [
        { name: "Get", args: ["s", "s"], returns: "v" },
        { name: "GetAll", args: ["s"], returns: "a{sv}" },
        { name: "Set", args: ["s", "s", "v"], returns: "" },
      ],
      properties: [],
      signals: [{ name: "PropertiesChanged", args: ["s", "a{sv}", "as"] }],
    },
  ],
  children: ["/org", "/org/freedesktop", "/org/freedesktop/NetworkManager/Devices"],
  schema_hash: "a3f8c1d2e4b567890abcdef1234567890abcdef1234567890abcdef12345678",
};

type Step = "select" | "inspect" | "persist";

const STEPS: { id: Step; label: string; icon: React.ElementType }[] = [
  { id: "select", label: "Select Object", icon: Search },
  { id: "inspect", label: "Introspect", icon: Eye },
  { id: "persist", label: "Send to DB", icon: Database },
];

export default function InspectorPage() {
  const [currentStep, setCurrentStep] = useState<Step>("select");
  const [selectedType, setSelectedType] = useState<string>("");
  const [selectedSource, setSelectedSource] = useState<string>("");
  const [inspecting, setInspecting] = useState(false);
  const [persisting, setPersisting] = useState(false);
  const [persisted, setPersisted] = useState(false);

  const stepIndex = STEPS.findIndex((s) => s.id === currentStep);
  const sources = MOCK_TARGETS.find((t) => t.id === selectedType)?.sources ?? [];

  const handleIntrospect = () => {
    setInspecting(true);
    setTimeout(() => {
      setInspecting(false);
      setCurrentStep("inspect");
    }, 1200);
  };

  const handlePersist = () => {
    setPersisting(true);
    setTimeout(() => {
      setPersisting(false);
      setPersisted(true);
      setCurrentStep("persist");
    }, 900);
  };

  const handleReset = () => {
    setCurrentStep("select");
    setSelectedType("");
    setSelectedSource("");
    setPersisted(false);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-border bg-card/50">
        <div>
          <h1 className="text-lg font-semibold tracking-tight text-foreground">
            Inspector Gadget
          </h1>
          <p className="text-xs text-muted-foreground mt-0.5">
            Introspect objects → review results → persist to DB
          </p>
        </div>
        {currentStep !== "select" && (
          <Button variant="outline" size="sm" onClick={handleReset}>
            Start Over
          </Button>
        )}
      </header>

      {/* Stepper */}
      <div className="flex items-center gap-2 px-6 py-3 bg-muted/30 border-b border-border">
        {STEPS.map((step, i) => {
          const done = i < stepIndex || (i === 2 && persisted);
          const active = step.id === currentStep && !persisted;
          return (
            <div key={step.id} className="flex items-center gap-2">
              {i > 0 && (
                <ChevronRight className="h-3 w-3 text-muted-foreground/40" />
              )}
              <div
                className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium transition-colors ${
                  done
                    ? "text-primary bg-primary/10"
                    : active
                    ? "text-foreground bg-accent/20"
                    : "text-muted-foreground"
                }`}
              >
                {done ? (
                  <CheckCircle2 className="h-3.5 w-3.5" />
                ) : active ? (
                  <step.icon className="h-3.5 w-3.5" />
                ) : (
                  <Circle className="h-3.5 w-3.5" />
                )}
                {step.label}
              </div>
            </div>
          );
        })}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {/* Step 1: Select */}
        {currentStep === "select" && (
          <div className="max-w-lg mx-auto space-y-6">
            <Card>
              <CardHeader>
                <CardTitle className="text-sm">Choose Target Type</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <Select value={selectedType} onValueChange={(v) => { setSelectedType(v); setSelectedSource(""); }}>
                  <SelectTrigger>
                    <SelectValue placeholder="Select introspection target…" />
                  </SelectTrigger>
                  <SelectContent>
                    {MOCK_TARGETS.map((t) => (
                      <SelectItem key={t.id} value={t.id}>
                        {t.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>

                {sources.length > 0 && (
                  <>
                    <Separator />
                    <Select value={selectedSource} onValueChange={setSelectedSource}>
                      <SelectTrigger>
                        <SelectValue placeholder="Select object…" />
                      </SelectTrigger>
                      <SelectContent>
                        {sources.map((s) => (
                          <SelectItem key={s.id} value={s.id}>
                            <span className="font-mono text-xs">{s.label}</span>
                            <span className="ml-2 text-muted-foreground text-[10px]">{s.path}</span>
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </>
                )}

                <Button
                  className="w-full"
                  disabled={!selectedSource || inspecting}
                  onClick={handleIntrospect}
                >
                  {inspecting ? (
                    <>
                      <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                      Introspecting…
                    </>
                  ) : (
                    <>
                      Introspect
                      <ArrowRight className="h-4 w-4 ml-2" />
                    </>
                  )}
                </Button>
              </CardContent>
            </Card>
          </div>
        )}

        {/* Step 2: Display Results */}
        {currentStep === "inspect" && (
          <div className="max-w-3xl mx-auto space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold text-foreground">
                  Introspection Result
                </h2>
                <p className="text-xs text-muted-foreground font-mono mt-0.5">
                  {MOCK_RESULT.name} — {MOCK_RESULT.path}
                </p>
              </div>
              <Button onClick={handlePersist} disabled={persisting}>
                {persisting ? (
                  <>
                    <Loader2 className="h-4 w-4 mr-2 animate-spin" />
                    Sending…
                  </>
                ) : (
                  <>
                    <Database className="h-4 w-4 mr-2" />
                    Send to DB
                  </>
                )}
              </Button>
            </div>

            {MOCK_RESULT.interfaces.map((iface) => (
              <Card key={iface.name}>
                <CardHeader className="pb-2">
                  <CardTitle className="text-xs font-mono flex items-center gap-2">
                    {iface.name}
                    <Badge variant="secondary" className="text-[10px]">
                      {iface.methods.length}m / {iface.properties.length}p / {iface.signals.length}s
                    </Badge>
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  {/* Methods */}
                  {iface.methods.length > 0 && (
                    <div>
                      <p className="text-[10px] uppercase tracking-widest text-muted-foreground mb-1.5">
                        Methods
                      </p>
                      <div className="space-y-1">
                        {iface.methods.map((m) => (
                          <div
                            key={m.name}
                            className="flex items-center gap-2 text-xs font-mono px-2 py-1 rounded bg-muted/50"
                          >
                            <span className="text-primary font-medium">{m.name}</span>
                            <span className="text-muted-foreground">
                              ({m.args.join(", ")})
                            </span>
                            {m.returns && (
                              <span className="text-accent ml-auto">→ {m.returns}</span>
                            )}
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Properties */}
                  {iface.properties.length > 0 && (
                    <div>
                      <p className="text-[10px] uppercase tracking-widest text-muted-foreground mb-1.5">
                        Properties
                      </p>
                      <div className="space-y-1">
                        {iface.properties.map((p) => (
                          <div
                            key={p.name}
                            className="flex items-center gap-2 text-xs font-mono px-2 py-1 rounded bg-muted/50"
                          >
                            <span className="text-foreground">{p.name}</span>
                            <Badge variant="outline" className="text-[10px] h-4">
                              {p.type}
                            </Badge>
                            <span className="text-muted-foreground text-[10px] ml-auto">
                              {p.access}
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Signals */}
                  {iface.signals.length > 0 && (
                    <div>
                      <p className="text-[10px] uppercase tracking-widest text-muted-foreground mb-1.5">
                        Signals
                      </p>
                      <div className="space-y-1">
                        {iface.signals.map((s) => (
                          <div
                            key={s.name}
                            className="flex items-center gap-2 text-xs font-mono px-2 py-1 rounded bg-muted/50"
                          >
                            <span className="text-warning">{s.name}</span>
                            <span className="text-muted-foreground">
                              ({s.args.join(", ")})
                            </span>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}
                </CardContent>
              </Card>
            ))}

            {/* Children */}
            <Card>
              <CardHeader className="pb-2">
                <CardTitle className="text-xs">Child Nodes</CardTitle>
              </CardHeader>
              <CardContent>
                <div className="space-y-1">
                  {MOCK_RESULT.children.map((c) => (
                    <div
                      key={c}
                      className="text-xs font-mono text-muted-foreground px-2 py-1 rounded bg-muted/50 hover:text-foreground transition-colors cursor-pointer"
                    >
                      {c}
                    </div>
                  ))}
                </div>
              </CardContent>
            </Card>

            {/* Schema Hash */}
            <div className="text-[10px] font-mono text-muted-foreground text-center">
              sha256: {MOCK_RESULT.schema_hash}
            </div>
          </div>
        )}

        {/* Step 3: Persisted */}
        {currentStep === "persist" && persisted && (
          <div className="max-w-lg mx-auto text-center space-y-4 py-12">
            <div className="flex items-center justify-center">
              <div className="h-16 w-16 rounded-full bg-primary/10 flex items-center justify-center">
                <CheckCircle2 className="h-8 w-8 text-primary" />
              </div>
            </div>
            <div>
              <h2 className="text-sm font-semibold text-foreground">
                Persisted to Database
              </h2>
              <p className="text-xs text-muted-foreground mt-1 font-mono">
                {MOCK_RESULT.name}
              </p>
            </div>
            <div className="bg-muted/50 rounded-lg p-4 text-left space-y-2">
              <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">State Key</span>
                <span className="font-mono text-foreground">
                  dbus/org_freedesktop_NetworkManager/_
                </span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">Schema Hash</span>
                <span className="font-mono text-foreground truncate ml-4 max-w-[280px]">
                  {MOCK_RESULT.schema_hash}
                </span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">Interfaces</span>
                <span className="font-mono text-foreground">
                  {MOCK_RESULT.interfaces.length}
                </span>
              </div>
              <div className="flex justify-between text-xs">
                <span className="text-muted-foreground">Blockchain Event</span>
                <Badge variant="secondary" className="text-[10px]">
                  dbus.schema.update
                </Badge>
              </div>
            </div>
            <Button variant="outline" onClick={handleReset} className="mt-4">
              Introspect Another
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
