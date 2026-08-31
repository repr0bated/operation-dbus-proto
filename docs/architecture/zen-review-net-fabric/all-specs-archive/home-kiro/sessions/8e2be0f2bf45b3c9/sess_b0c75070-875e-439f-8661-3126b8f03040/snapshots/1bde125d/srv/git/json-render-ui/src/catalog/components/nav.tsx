interface NavItemProps {
  props: {
    label: string;
    route: string;
    icon?: string | null;
    active?: boolean | null;
  };
  emit?: (event: string) => void;
}

export function NavItem({ props, emit }: NavItemProps) {
  const active = props.active ?? false;
  return (
    <button
      onClick={() => emit?.("press")}
      className={`w-full text-left px-3 py-1.5 text-sm rounded-md mx-1 ${
        active
          ? "bg-neutral-700 text-neutral-100 font-medium"
          : "text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200"
      }`}
    >
      {props.label}
    </button>
  );
}
