import os
import shutil
import json

# Create a test directory
test_dir = "test_models_init"
if os.path.exists(test_dir):
    shutil.rmtree(test_dir)
os.makedirs(test_dir)

# Change to the test directory
os.chdir(test_dir)

# Run the ragit init command
print("Running 'rag init'...")
os.system("$HOME/.cargo/bin/rag init")

print("\n--- Testing with default model ---")

# Check if the models.json file exists
models_json_path = os.path.join(".ragit", "models.json")
if os.path.exists(models_json_path):
    print(f"models.json file exists at: {models_json_path}")
    
    # Read the content of the models.json file
    with open(models_json_path, 'r') as f:
        models_json = json.load(f)
    
    print(f"Number of models in models.json: {len(models_json)}")
    print(f"Model names: {[model['name'] for model in models_json]}")
else:
    print(f"models.json file does not exist at: {models_json_path}")

# Check if the api.json file exists
api_json_path = os.path.join(".ragit", "configs", "api.json")
if os.path.exists(api_json_path):
    print(f"api.json file exists at: {api_json_path}")
    
    # Read the content of the api.json file
    with open(api_json_path, 'r') as f:
        api_json = json.load(f)
    
    print(f"Selected model in api.json: {api_json.get('model', 'Not found')}")
    
    # Check if the selected model exists in models.json
    model_names = [model['name'] for model in models_json]
    if api_json.get('model') in model_names:
        print(f"Selected model '{api_json.get('model')}' exists in models.json")
    else:
        print(f"Selected model '{api_json.get('model')}' does NOT exist in models.json")
else:
    print(f"api.json file does not exist at: {api_json_path}")

# Now test with a custom models.json file without the default model
print("\n--- Testing with custom models.json without default model ---")

# Clean up the previous test
os.chdir("..")
shutil.rmtree(test_dir)

# Create a new test directory
test_dir = "test_models_init_custom"
if os.path.exists(test_dir):
    shutil.rmtree(test_dir)
os.makedirs(test_dir)

# Change to the test directory
os.chdir(test_dir)

# Create a custom models.json file without the default model (llama3.3-70b-groq)
custom_models = [
    {
        "name": "gpt-4o",
        "api_name": "gpt-4o",
        "can_read_images": True,
        "api_provider": "openai",
        "api_url": "https://api.openai.com/v1/chat/completions",
        "input_price": 2.5,
        "output_price": 10.0,
        "api_timeout": None,
        "explanation": None,
        "api_key": None,
        "api_env_var": "OPENAI_API_KEY"
    },
    {
        "name": "gpt-4o-mini",
        "api_name": "gpt-4o-mini",
        "can_read_images": True,
        "api_provider": "openai",
        "api_url": "https://api.openai.com/v1/chat/completions",
        "input_price": 0.15,
        "output_price": 0.6,
        "api_timeout": None,
        "explanation": None,
        "api_key": None,
        "api_env_var": "OPENAI_API_KEY"
    },
    {
        "name": "phi-4-14b-ollama",
        "api_name": "phi4:14b",
        "can_read_images": True,
        "api_provider": "openai",
        "api_url": "http://127.0.0.1:11434/v1/chat/completions",
        "input_price": 0.0,
        "output_price": 0.0,
        "api_timeout": None,
        "explanation": None,
        "api_key": None,
        "api_env_var": None
    }
]

# Create a temporary models.json file
custom_models_path = os.path.join(os.getcwd(), "custom_models.json")
with open(custom_models_path, 'w') as f:
    json.dump(custom_models, f, indent=2)

# Run the ragit init command with the custom models.json file
print("Running 'rag init' with custom models.json...")
os.system(f"RAGIT_MODEL_CONFIG={custom_models_path} $HOME/.cargo/bin/rag init")

# Check if the models.json file exists
models_json_path = os.path.join(".ragit", "models.json")
if os.path.exists(models_json_path):
    print(f"models.json file exists at: {models_json_path}")
    
    # Read the content of the models.json file
    with open(models_json_path, 'r') as f:
        models_json = json.load(f)
    
    print(f"Number of models in models.json: {len(models_json)}")
    print(f"Model names: {[model['name'] for model in models_json]}")
else:
    print(f"models.json file does not exist at: {models_json_path}")

# Check if the api.json file exists
api_json_path = os.path.join(".ragit", "configs", "api.json")
if os.path.exists(api_json_path):
    print(f"api.json file exists at: {api_json_path}")
    
    # Read the content of the api.json file
    with open(api_json_path, 'r') as f:
        api_json = json.load(f)
    
    print(f"Selected model in api.json: {api_json.get('model', 'Not found')}")
    
    # Check if the selected model exists in models.json
    model_names = [model['name'] for model in models_json]
    if api_json.get('model') in model_names:
        print(f"Selected model '{api_json.get('model')}' exists in models.json")
        
        # Check if it's the lowest-cost model (phi-4-14b-ollama)
        if api_json.get('model') == 'phi-4-14b-ollama':
            print("SUCCESS: The lowest-cost model was correctly selected!")
        else:
            print(f"FAILURE: The selected model is not the lowest-cost model (expected 'phi-4-14b-ollama')")
    else:
        print(f"Selected model '{api_json.get('model')}' does NOT exist in models.json")
else:
    print(f"api.json file does not exist at: {api_json_path}")

# Clean up
os.chdir("..")
shutil.rmtree(test_dir)
