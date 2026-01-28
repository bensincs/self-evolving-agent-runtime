# Capability: Greeting

## Task
Return a friendly greeting to the provided input text. Input JSON: {"text": string}. Output JSON: {"greeting": string} where greeting is like 'Hello, <text>!'.

## Response Fields
- `greeting` (string): Friendly greeting including the input text.

## Database Fields to Read
- None

## Test Cases
- Input text "World" should return greeting "Hello, World!"
- Input text "Alice" should return greeting "Hello, Alice!"