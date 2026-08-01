import { useState } from "react";
import { Plus, Play, Pause, Trash2, Clock, MoreVertical, Calendar } from "lucide-react";
import { Card, CardContent } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

interface CronJob {
  id: string;
  name: string;
  schedule: string;
  prompt: string;
  enabled: boolean;
  delivery: "announce" | "webhook" | "none";
  lastRun?: string;
  lastStatus?: "success" | "error" | "skipped";
  nextRun: string;
  agent?: string;
}

const initialJobs: CronJob[] = [
  { id: "1", name: "Morning Briefing", schedule: "0 8 * * *", prompt: "Give me a morning briefing of emails, calendar, and news", enabled: true, delivery: "announce", lastRun: "8:00 AM", lastStatus: "success", nextRun: "Tomorrow 8:00 AM" },
  { id: "2", name: "Check Server Health", schedule: "*/15 * * * *", prompt: "Check server health metrics and alert if anything is off", enabled: true, delivery: "webhook", lastRun: "2m ago", lastStatus: "success", nextRun: "In 13m" },
  { id: "3", name: "Weekly Report", schedule: "0 17 * * 5", prompt: "Generate a weekly summary of all tasks completed", enabled: true, delivery: "announce", lastRun: "Last Friday", lastStatus: "success", nextRun: "Friday 5:00 PM" },
  { id: "4", name: "Backup Reminders", schedule: "0 22 * * *", prompt: "Check if today's backups completed successfully", enabled: false, delivery: "none", lastRun: "3d ago", lastStatus: "skipped", nextRun: "Disabled" },
  { id: "5", name: "News Digest", schedule: "0 12 * * *", prompt: "Search for the latest AI and tech news and summarize", enabled: true, delivery: "announce", lastRun: "12:00 PM", lastStatus: "error", nextRun: "Tomorrow 12:00 PM", agent: "researcher" },
];

const statusColors: Record<string, string> = {
  success: "text-green-600 dark:text-green-400 bg-green-500/10",
  error: "text-red-600 dark:text-red-400 bg-red-500/10",
  skipped: "text-muted-foreground bg-muted",
};

export default function CronPage() {
  const [jobs, setJobs] = useState(initialJobs);

  const toggleJob = (id: string) => {
    setJobs((prev) => prev.map((j) => (j.id === id ? { ...j, enabled: !j.enabled } : j)));
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Cron Jobs</h1>
          <p className="text-sm text-muted-foreground mt-1">
            Schedule recurring tasks with delivery controls
          </p>
        </div>
        <Button size="sm" className="gap-1.5">
          <Plus className="h-4 w-4" /> New Job
        </Button>
      </div>

      <div className="grid gap-3">
        {jobs.map((job) => (
          <Card key={job.id} className="bg-card border-border">
            <CardContent className="p-4">
              <div className="flex items-center justify-between">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-sm text-foreground">{job.name}</span>
                    <Badge variant="outline" className="font-mono text-[10px] h-5">{job.schedule}</Badge>
                    <Badge variant="secondary" className="text-[10px] h-5">{job.delivery}</Badge>
                    {job.agent && (
                      <Badge variant="secondary" className="text-[10px] h-5">agent: {job.agent}</Badge>
                    )}
                  </div>
                  <p className="text-xs text-muted-foreground mt-1 truncate max-w-lg">{job.prompt}</p>
                  <div className="flex items-center gap-3 mt-2 text-xs text-muted-foreground">
                    <span className="flex items-center gap-1">
                      <Clock className="h-3 w-3" />
                      Last: {job.lastRun || "Never"}
                    </span>
                    {job.lastStatus && (
                      <Badge className={`text-[10px] h-5 ${statusColors[job.lastStatus]}`} variant="secondary">
                        {job.lastStatus}
                      </Badge>
                    )}
                    <span className="flex items-center gap-1">
                      <Calendar className="h-3 w-3" />
                      Next: {job.nextRun}
                    </span>
                  </div>
                </div>

                <div className="flex items-center gap-3 shrink-0 ml-4">
                  <Button variant="outline" size="icon" className="h-7 w-7">
                    <Play className="h-3 w-3" />
                  </Button>
                  <Switch checked={job.enabled} onCheckedChange={() => toggleJob(job.id)} />
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button variant="ghost" size="icon" className="h-7 w-7">
                        <MoreVertical className="h-4 w-4" />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuItem className="text-sm">Edit</DropdownMenuItem>
                      <DropdownMenuItem className="text-sm">Run History</DropdownMenuItem>
                      <DropdownMenuItem className="text-sm">Advanced</DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem className="text-sm text-destructive">
                        <Trash2 className="h-3.5 w-3.5 mr-2" /> Delete
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  );
}
