import { AppHeader } from "@/components/layout/AppHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Shield, Link2 } from "lucide-react";

export default function SecurityPage() {
  return (
    <>
      <AppHeader title="Security" subtitle="audit & access" />
      <div className="flex-1 overflow-auto p-4 md:p-6 space-y-4 max-w-3xl">
        <Card className="bg-card border-border">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Shield className="h-4 w-4 text-primary" />
              Blockchain Audit Trail
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-xs text-muted-foreground font-mono">
              All mutations are tracked through the event chain with cryptographic verification.
              Audit log explorer will connect to /api/events SSE stream.
            </p>
          </CardContent>
        </Card>

        <Card className="bg-card border-border">
          <CardHeader className="pb-2">
            <CardTitle className="text-sm font-medium flex items-center gap-2">
              <Link2 className="h-4 w-4 text-accent" />
              WireGuard Sessions
            </CardTitle>
          </CardHeader>
          <CardContent>
            <p className="text-xs text-muted-foreground font-mono">
              X25519 key exchange · ChaCha20-Poly1305 AEAD · Argon2id KDF
              <br />
              Session management via op-gateway smart routing.
            </p>
          </CardContent>
        </Card>
      </div>
    </>
  );
}
