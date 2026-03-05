// Reusable neutral chip row keeps filters lightweight; active chips use outline emphasis without filled backgrounds.
export type FilterChipOption<T extends string = string> = {
  id: T;
  label: string;
  tone?: "default" | "warning";
};

export interface FilterChipsProps<T extends string = string> {
  options: Array<FilterChipOption<T>>;
  value: T;
  onChange: (value: T) => void;
  className?: string;
}

export default function FilterChips<T extends string = string>(props: FilterChipsProps<T>) {
  return (
    <div className={props.className ?? "activityFilters"}>
      {props.options.map((option) => {
        const active = props.value === option.id;
        return (
          <button
            key={option.id}
            type="button"
            className={`chip chipButton activityFilterChip ${active ? "active" : "subtle"} ${
              option.tone === "warning" ? "warnings" : ""
            }`}
            aria-pressed={active}
            onClick={() => props.onChange(option.id)}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

