import type { HostedAiProvider } from "../types";
import type { AssistantSelectOption } from "./AssistantSelect";

export const HOSTED_PROVIDER_OPTIONS: ReadonlyArray<AssistantSelectOption<HostedAiProvider>> = [
  { value: "openai", label: "OpenAI" },
  { value: "anthropic", label: "Anthropic" },
];

export const HOSTED_MODEL_OPTIONS: Record<
  HostedAiProvider,
  ReadonlyArray<AssistantSelectOption<string>>
> = {
  openai: [
    { value: "gpt-5.6-luna", label: "GPT-5.6 Luna" },
    { value: "gpt-5.6-terra", label: "GPT-5.6 Terra" },
    { value: "gpt-5.6-sol", label: "GPT-5.6 Sol" },
  ],
  anthropic: [
    { value: "claude-sonnet-5", label: "Claude Sonnet 5" },
    { value: "claude-opus-5", label: "Claude Opus 5" },
    { value: "claude-fable-5", label: "Claude Fable 5" },
    { value: "claude-haiku-4-5-20251001", label: "Claude Haiku 4.5" },
  ],
};

export const DEFAULT_HOSTED_MODEL: Record<HostedAiProvider, string> = {
  openai: "gpt-5.6-luna",
  anthropic: "claude-sonnet-5",
};
