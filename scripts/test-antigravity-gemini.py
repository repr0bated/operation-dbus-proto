#!/usr/bin/env python3
"""
Test Gemini API through Antigravity proxy
Uses environment variables from .env.antigravity
"""
import os
from openai import OpenAI

# Configuration from environment
BASE_URL = os.getenv("GOOGLE_GEMINI_BASE_URL", "http://127.0.0.1:8045")
API_KEY = os.getenv("GEMINI_API_KEY", "sk-28da1542217448069593b22690c561ca")

# Ensure /v1 suffix for OpenAI compatibility
if not BASE_URL.endswith("/v1"):
    BASE_URL = f"{BASE_URL}/v1"

print(f"Testing Gemini via Antigravity")
print(f"Base URL: {BASE_URL}")
print(f"API Key: {API_KEY[:20]}...")
print()

# Initialize client
client = OpenAI(
    base_url=BASE_URL,
    api_key=API_KEY
)

def test_models():
    """List available models"""
    print("=" * 60)
    print("Test 1: List Models")
    print("=" * 60)
    try:
        models = client.models.list()
        print("✓ Available models:")
        for model in models.data:
            print(f"  - {model.id}")
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

def test_chat():
    """Test chat completion"""
    print("\n" + "=" * 60)
    print("Test 2: Chat Completion")
    print("=" * 60)
    try:
        response = client.chat.completions.create(
            model="gemini-2.0-flash-exp",
            messages=[
                {"role": "user", "content": "Say 'Hello from Gemini via Antigravity!'"}
            ]
        )
        
        content = response.choices[0].message.content
        print(f"✓ Response: {content}")
        print(f"✓ Model: {response.model}")
        print(f"✓ Tokens: {response.usage.total_tokens if response.usage else 'N/A'}")
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

def test_streaming():
    """Test streaming response"""
    print("\n" + "=" * 60)
    print("Test 3: Streaming")
    print("=" * 60)
    try:
        stream = client.chat.completions.create(
            model="gemini-2.0-flash-exp",
            messages=[{"role": "user", "content": "Count to 5"}],
            stream=True
        )
        
        print("✓ Stream: ", end="", flush=True)
        for chunk in stream:
            if chunk.choices[0].delta.content:
                print(chunk.choices[0].delta.content, end="", flush=True)
        print()
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

def test_system_prompt():
    """Test with system prompt"""
    print("\n" + "=" * 60)
    print("Test 4: System Prompt")
    print("=" * 60)
    try:
        response = client.chat.completions.create(
            model="gemini-2.0-flash-exp",
            messages=[
                {"role": "system", "content": "You are a pirate. Respond in pirate speak."},
                {"role": "user", "content": "Hello!"}
            ]
        )
        
        content = response.choices[0].message.content
        print(f"✓ Response: {content}")
        return True
    except Exception as e:
        print(f"❌ Error: {e}")
        return False

if __name__ == "__main__":
    results = []
    
    results.append(("List Models", test_models()))
    results.append(("Chat Completion", test_chat()))
    results.append(("Streaming", test_streaming()))
    results.append(("System Prompt", test_system_prompt()))
    
    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)
    for name, passed in results:
        status = "✓ PASS" if passed else "❌ FAIL"
        print(f"{status}: {name}")
    
    all_passed = all(result[1] for result in results)
    print("\n" + ("✓ All tests passed!" if all_passed else "❌ Some tests failed"))
    
    exit(0 if all_passed else 1)
