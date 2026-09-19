import { Select as SelectPrimitive } from "@base-ui/react/select";
import { CaretDown, Check } from "@phosphor-icons/react";

export interface AssistantSelectOption<Value extends string> {
  value: Value;
  label: string;
}

interface AssistantSelectProps<Value extends string> {
  label: string;
  value: Value;
  options: ReadonlyArray<AssistantSelectOption<Value>>;
  onValueChange: (value: Value) => void;
  disabled: boolean;
  describedBy: string;
  popupLabel: string;
}

export function AssistantSelect<Value extends string>({
  label,
  value,
  options,
  onValueChange,
  disabled,
  describedBy,
  popupLabel,
}: AssistantSelectProps<Value>) {
  return (
    <SelectPrimitive.Root
      items={options}
      value={value}
      onValueChange={(nextValue) => {
        if (nextValue) onValueChange(nextValue);
      }}
      disabled={disabled}
    >
      <SelectPrimitive.Label className="assistant-select-label">
        {label}
      </SelectPrimitive.Label>
      <SelectPrimitive.Trigger
        className="assistant-select-trigger"
        aria-describedby={describedBy}
      >
        <SelectPrimitive.Value />
        <SelectPrimitive.Icon className="assistant-select-trigger-icon">
          <CaretDown size={13} weight="bold" aria-hidden="true" />
        </SelectPrimitive.Icon>
      </SelectPrimitive.Trigger>

      <SelectPrimitive.Portal>
        <SelectPrimitive.Positioner
          className="assistant-select-positioner"
          side="bottom"
          align="start"
          sideOffset={4}
          collisionPadding={8}
          alignItemWithTrigger={false}
        >
          <SelectPrimitive.Popup className="assistant-select-popup">
            <SelectPrimitive.List className="assistant-select-list" aria-label={popupLabel}>
              {options.map((option) => (
                <SelectPrimitive.Item
                  key={option.value}
                  value={option.value}
                  label={option.label}
                  className="assistant-select-item"
                >
                  <SelectPrimitive.ItemText>{option.label}</SelectPrimitive.ItemText>
                  <SelectPrimitive.ItemIndicator className="assistant-select-item-indicator">
                    <Check size={13} weight="bold" aria-hidden="true" />
                  </SelectPrimitive.ItemIndicator>
                </SelectPrimitive.Item>
              ))}
            </SelectPrimitive.List>
          </SelectPrimitive.Popup>
        </SelectPrimitive.Positioner>
      </SelectPrimitive.Portal>
    </SelectPrimitive.Root>
  );
}
