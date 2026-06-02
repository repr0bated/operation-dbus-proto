export type Json =
  | string
  | number
  | boolean
  | null
  | { [key: string]: Json | undefined }
  | Json[]

export type Database = {
  // Allows to automatically instantiate createClient with right options
  // instead of createClient<Database, { PostgrestVersion: 'XX' }>(URL, KEY)
  __InternalSupabase: {
    PostgrestVersion: "14.1"
  }
  public: {
    Tables: {
      alerts: {
        Row: {
          acknowledged_at: string | null
          acknowledged_by: string | null
          created_at: string
          description: string | null
          id: string
          node_id: string | null
          resolved_at: string | null
          severity: Database["public"]["Enums"]["alert_severity"]
          status: Database["public"]["Enums"]["alert_status"]
          title: string
        }
        Insert: {
          acknowledged_at?: string | null
          acknowledged_by?: string | null
          created_at?: string
          description?: string | null
          id?: string
          node_id?: string | null
          resolved_at?: string | null
          severity?: Database["public"]["Enums"]["alert_severity"]
          status?: Database["public"]["Enums"]["alert_status"]
          title: string
        }
        Update: {
          acknowledged_at?: string | null
          acknowledged_by?: string | null
          created_at?: string
          description?: string | null
          id?: string
          node_id?: string | null
          resolved_at?: string | null
          severity?: Database["public"]["Enums"]["alert_severity"]
          status?: Database["public"]["Enums"]["alert_status"]
          title?: string
        }
        Relationships: [
          {
            foreignKeyName: "alerts_node_id_fkey"
            columns: ["node_id"]
            isOneToOne: false
            referencedRelation: "nodes"
            referencedColumns: ["id"]
          },
        ]
      }
      dbus_objects: {
        Row: {
          created_at: string
          id: string
          interfaces: string[] | null
          path: string
          service_id: string | null
        }
        Insert: {
          created_at?: string
          id?: string
          interfaces?: string[] | null
          path: string
          service_id?: string | null
        }
        Update: {
          created_at?: string
          id?: string
          interfaces?: string[] | null
          path?: string
          service_id?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "dbus_objects_service_id_fkey"
            columns: ["service_id"]
            isOneToOne: false
            referencedRelation: "services"
            referencedColumns: ["id"]
          },
        ]
      }
      logs: {
        Row: {
          id: string
          level: string
          message: string
          metadata: Json | null
          node_id: string | null
          source: string | null
          timestamp: string
        }
        Insert: {
          id?: string
          level: string
          message: string
          metadata?: Json | null
          node_id?: string | null
          source?: string | null
          timestamp?: string
        }
        Update: {
          id?: string
          level?: string
          message?: string
          metadata?: Json | null
          node_id?: string | null
          source?: string | null
          timestamp?: string
        }
        Relationships: [
          {
            foreignKeyName: "logs_node_id_fkey"
            columns: ["node_id"]
            isOneToOne: false
            referencedRelation: "nodes"
            referencedColumns: ["id"]
          },
        ]
      }
      nodes: {
        Row: {
          created_at: string
          hostname: string
          id: string
          ip_address: unknown
          last_seen: string | null
          metadata: Json | null
          status: string | null
        }
        Insert: {
          created_at?: string
          hostname: string
          id?: string
          ip_address?: unknown
          last_seen?: string | null
          metadata?: Json | null
          status?: string | null
        }
        Update: {
          created_at?: string
          hostname?: string
          id?: string
          ip_address?: unknown
          last_seen?: string | null
          metadata?: Json | null
          status?: string | null
        }
        Relationships: []
      }
      saved_queries: {
        Row: {
          created_at: string
          id: string
          is_shared: boolean | null
          name: string
          query_filter: Json
          user_id: string
        }
        Insert: {
          created_at?: string
          id?: string
          is_shared?: boolean | null
          name: string
          query_filter: Json
          user_id: string
        }
        Update: {
          created_at?: string
          id?: string
          is_shared?: boolean | null
          name?: string
          query_filter?: Json
          user_id?: string
        }
        Relationships: []
      }
      services: {
        Row: {
          bus: Database["public"]["Enums"]["bus_type"]
          cmdline: string | null
          created_at: string
          id: string
          is_activatable: boolean | null
          name: string
          node_id: string | null
          pid: number | null
          unique_name: string | null
          updated_at: string
        }
        Insert: {
          bus?: Database["public"]["Enums"]["bus_type"]
          cmdline?: string | null
          created_at?: string
          id?: string
          is_activatable?: boolean | null
          name: string
          node_id?: string | null
          pid?: number | null
          unique_name?: string | null
          updated_at?: string
        }
        Update: {
          bus?: Database["public"]["Enums"]["bus_type"]
          cmdline?: string | null
          created_at?: string
          id?: string
          is_activatable?: boolean | null
          name?: string
          node_id?: string | null
          pid?: number | null
          unique_name?: string | null
          updated_at?: string
        }
        Relationships: [
          {
            foreignKeyName: "services_node_id_fkey"
            columns: ["node_id"]
            isOneToOne: false
            referencedRelation: "nodes"
            referencedColumns: ["id"]
          },
        ]
      }
      traces: {
        Row: {
          bus: Database["public"]["Enums"]["bus_type"]
          destination: string | null
          id: string
          interface: string | null
          member: string | null
          message_type: string
          node_id: string | null
          path: string | null
          payload: Json | null
          reply_serial: number | null
          sender: string | null
          serial: number | null
          signature: string | null
          timestamp: string
        }
        Insert: {
          bus: Database["public"]["Enums"]["bus_type"]
          destination?: string | null
          id?: string
          interface?: string | null
          member?: string | null
          message_type: string
          node_id?: string | null
          path?: string | null
          payload?: Json | null
          reply_serial?: number | null
          sender?: string | null
          serial?: number | null
          signature?: string | null
          timestamp?: string
        }
        Update: {
          bus?: Database["public"]["Enums"]["bus_type"]
          destination?: string | null
          id?: string
          interface?: string | null
          member?: string | null
          message_type?: string
          node_id?: string | null
          path?: string | null
          payload?: Json | null
          reply_serial?: number | null
          sender?: string | null
          serial?: number | null
          signature?: string | null
          timestamp?: string
        }
        Relationships: [
          {
            foreignKeyName: "traces_node_id_fkey"
            columns: ["node_id"]
            isOneToOne: false
            referencedRelation: "nodes"
            referencedColumns: ["id"]
          },
        ]
      }
      user_keypairs: {
        Row: {
          created_at: string
          id: string
          private_key_encrypted: string
          public_key: string
          user_id: string
        }
        Insert: {
          created_at?: string
          id?: string
          private_key_encrypted: string
          public_key: string
          user_id: string
        }
        Update: {
          created_at?: string
          id?: string
          private_key_encrypted?: string
          public_key?: string
          user_id?: string
        }
        Relationships: []
      }
      user_roles: {
        Row: {
          created_at: string
          id: string
          role: Database["public"]["Enums"]["app_role"]
          user_id: string
        }
        Insert: {
          created_at?: string
          id?: string
          role?: Database["public"]["Enums"]["app_role"]
          user_id: string
        }
        Update: {
          created_at?: string
          id?: string
          role?: Database["public"]["Enums"]["app_role"]
          user_id?: string
        }
        Relationships: []
      }
    }
    Views: {
      [_ in never]: never
    }
    Functions: {
      get_user_role: {
        Args: { _user_id: string }
        Returns: Database["public"]["Enums"]["app_role"]
      }
      has_role: {
        Args: {
          _role: Database["public"]["Enums"]["app_role"]
          _user_id: string
        }
        Returns: boolean
      }
      setup_new_user: {
        Args: {
          _private_key_encrypted: string
          _public_key: string
          _user_id: string
        }
        Returns: undefined
      }
      show_limit: { Args: never; Returns: number }
      show_trgm: { Args: { "": string }; Returns: string[] }
    }
    Enums: {
      alert_severity: "critical" | "warning" | "info"
      alert_status: "open" | "acknowledged" | "resolved"
      app_role: "admin" | "operator" | "user"
      bus_type: "system" | "session"
    }
    CompositeTypes: {
      [_ in never]: never
    }
  }
}

type DatabaseWithoutInternals = Omit<Database, "__InternalSupabase">

type DefaultSchema = DatabaseWithoutInternals[Extract<keyof Database, "public">]

export type Tables<
  DefaultSchemaTableNameOrOptions extends
    | keyof (DefaultSchema["Tables"] & DefaultSchema["Views"])
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
        DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
      DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])[TableName] extends {
      Row: infer R
    }
    ? R
    : never
  : DefaultSchemaTableNameOrOptions extends keyof (DefaultSchema["Tables"] &
        DefaultSchema["Views"])
    ? (DefaultSchema["Tables"] &
        DefaultSchema["Views"])[DefaultSchemaTableNameOrOptions] extends {
        Row: infer R
      }
      ? R
      : never
    : never

export type TablesInsert<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Insert: infer I
    }
    ? I
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Insert: infer I
      }
      ? I
      : never
    : never

export type TablesUpdate<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Update: infer U
    }
    ? U
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Update: infer U
      }
      ? U
      : never
    : never

export type Enums<
  DefaultSchemaEnumNameOrOptions extends
    | keyof DefaultSchema["Enums"]
    | { schema: keyof DatabaseWithoutInternals },
  EnumName extends DefaultSchemaEnumNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"]
    : never = never,
> = DefaultSchemaEnumNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"][EnumName]
  : DefaultSchemaEnumNameOrOptions extends keyof DefaultSchema["Enums"]
    ? DefaultSchema["Enums"][DefaultSchemaEnumNameOrOptions]
    : never

export type CompositeTypes<
  PublicCompositeTypeNameOrOptions extends
    | keyof DefaultSchema["CompositeTypes"]
    | { schema: keyof DatabaseWithoutInternals },
  CompositeTypeName extends PublicCompositeTypeNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"]
    : never = never,
> = PublicCompositeTypeNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"][CompositeTypeName]
  : PublicCompositeTypeNameOrOptions extends keyof DefaultSchema["CompositeTypes"]
    ? DefaultSchema["CompositeTypes"][PublicCompositeTypeNameOrOptions]
    : never

export const Constants = {
  public: {
    Enums: {
      alert_severity: ["critical", "warning", "info"],
      alert_status: ["open", "acknowledged", "resolved"],
      app_role: ["admin", "operator", "user"],
      bus_type: ["system", "session"],
    },
  },
} as const
