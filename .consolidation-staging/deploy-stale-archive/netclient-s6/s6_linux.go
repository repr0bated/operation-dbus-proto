package daemon

import (
	"os"

	"github.com/gravitl/netclient/ncutils"
	"github.com/gravitl/netmaker/logger"
	"golang.org/x/exp/slog"
)

// s6Service returns the s6 service name that supervises `netclient daemon`.
// The s6 service tree is owned by the op-dbus deploy; netclient only drives its
// lifecycle. Override with NETCLIENT_S6_SERVICE.
func s6Service() string {
	if v := os.Getenv("NETCLIENT_S6_SERVICE"); v != "" {
		return v
	}
	return "netclient"
}

// s6Ctl runs an s6d verb against the netclient service. s6d is the D-Bus client
// for org.opdbus.v1.S6.Systemctl, which maps systemctl semantics onto Artix s6.
// Lifecycle ops MUST stay on this D-Bus control plane: never s6-rc, s6-svc, or
// systemctl directly.
func s6Ctl(verb string) error {
	_, err := ncutils.RunCmd("s6d "+verb+" "+s6Service(), false)
	return err
}

// setupS6 enables the deploy-owned s6 service through the D-Bus control plane.
func setupS6() error {
	slog.Info("enabling netclient s6 service via s6d", "service", s6Service())
	if _, err := ncutils.RunCmd("s6d daemon-reload", true); err != nil {
		logger.Log(0, "s6d daemon-reload failed", err.Error())
	}
	return s6Ctl("enable")
}

func startS6() error {
	logger.Log(3, "calling s6d start "+s6Service())
	return s6Ctl("start")
}

func stopS6() error {
	logger.Log(3, "calling s6d stop "+s6Service())
	return s6Ctl("stop")
}

func restartS6() error {
	slog.Info("restarting netclient s6 service via s6d", "service", s6Service())
	return s6Ctl("restart")
}

// removeS6 disables the service via s6d. The deploy owns the s6 service tree, so
// it is never deleted here.
func removeS6() error {
	slog.Info("disabling netclient s6 service via s6d", "service", s6Service())
	if err := s6Ctl("disable"); err != nil {
		return err
	}
	_, err := ncutils.RunCmd("s6d daemon-reload", false)
	return err
}
