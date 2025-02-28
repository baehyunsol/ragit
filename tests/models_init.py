import json
import os
import shutil
from utils import cargo_run, goto_root, mk_and_cd_tmp_dir, write_string

def models_init():
    """Test the initialization of models.json and model selection in api.json."""
    goto_root()
    
    # Test with default model
    print("\n--- Testing with default model ---")
    mk_and_cd_tmp_dir()
    
    # Run the ragit init command
    print("Running 'rag init'...")
    cargo_run(["init"])
    
    # Check if the models.json file exists
    models_json_path = os.path.join(".ragit", "models.json")
    assert os.path.exists(models_json_path), f"models.json file does not exist at: {models_json_path}"
    print(f"models.json file exists at: {models_json_path}")
    
    # Read the content of the models.json file
    with open(models_json_path, 'r') as f:
        models_json = json.load(f)
    
    print(f"Number of models in models.json: {len(models_json)}")
    model_names = [model['name'] for model in models_json]
    print(f"Model names: {model_names}")
    
    # Check if the api.json file exists
    api_json_path = os.path.join(".ragit", "configs", "api.json")
    assert os.path.exists(api_json_path), f"api.json file does not exist at: {api_json_path}"
    print(f"api.json file exists at: {api_json_path}")
    
    # Read the content of the api.json file
    with open(api_json_path, 'r') as f:
        api_json = json.load(f)
    
    selected_model = api_json.get('model', 'Not found')
    print(f"Selected model in api.json: {selected_model}")
    
    # Check if the selected model exists in models.json
    assert selected_model in model_names, f"Selected model '{selected_model}' does NOT exist in models.json"
    print(f"Selected model '{selected_model}' exists in models.json")
    
    # Clean up the previous test
    os.chdir("..")
    
    # Test with custom models.json without the default model
    print("\n--- Testing with custom models.json without default model ---")
    mk_and_cd_tmp_dir()
    
    # Create a custom models.json file without the default model
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
    os.environ["RAGIT_MODEL_CONFIG"] = custom_models_path
    cargo_run(["init"])
    
    # Check if the models.json file exists
    models_json_path = os.path.join(".ragit", "models.json")
    assert os.path.exists(models_json_path), f"models.json file does not exist at: {models_json_path}"
    print(f"models.json file exists at: {models_json_path}")
    
    # Read the content of the models.json file
    with open(models_json_path, 'r') as f:
        models_json = json.load(f)
    
    print(f"Number of models in models.json: {len(models_json)}")
    model_names = [model['name'] for model in models_json]
    print(f"Model names: {model_names}")
    assert len(models_json) == 3, f"Expected 3 models, got {len(models_json)}"
    
    # Check if the api.json file exists
    api_json_path = os.path.join(".ragit", "configs", "api.json")
    assert os.path.exists(api_json_path), f"api.json file does not exist at: {api_json_path}"
    print(f"api.json file exists at: {api_json_path}")
    
    # Read the content of the api.json file
    with open(api_json_path, 'r') as f:
        api_json = json.load(f)
    
    selected_model = api_json.get('model', 'Not found')
    print(f"Selected model in api.json: {selected_model}")
    
    # Check if the selected model exists in models.json
    assert selected_model in model_names, f"Selected model '{selected_model}' does NOT exist in models.json"
    print(f"Selected model '{selected_model}' exists in models.json")
    
    # Check if it's the lowest-cost model (phi-4-14b-ollama)
    assert selected_model == 'phi-4-14b-ollama', f"Expected lowest-cost model 'phi-4-14b-ollama', got '{selected_model}'"
    print("SUCCESS: The lowest-cost model was correctly selected!")
    
    # Clean up environment variable
    if "RAGIT_MODEL_CONFIG" in os.environ:
        del os.environ["RAGIT_MODEL_CONFIG"]

def test_home_config_override():
    """Test that ~/.config/ragit/*.json values override defaults when creating new config files."""
    goto_root()
    
    # Create a temporary ~/.config/ragit directory
    home_dir = os.path.expanduser("~")
    config_dir = os.path.join(home_dir, ".config", "ragit")
    os.makedirs(config_dir, exist_ok=True)
    
    # Create a custom api.json in ~/.config/ragit
    home_api_json = {
        "api_key": None,
        "model": "gpt-4o",  # Different from default
        "timeout": 240000,  # Different from default
        "sleep_between_retries": 30000,  # Different from default
        "max_retry": 10,  # Different from default
        "sleep_after_llm_call": 1000,  # Different from default
        "dump_log": True,  # Different from default
        "dump_api_usage": True
    }
    
    # Create a custom build.json in ~/.config/ragit with only a subset of fields
    home_build_json = {
        "chunk_size": 2000,  # Different from default
        "slide_len": 500,  # Different from default
        # Omitting other fields to test partial configuration
    }
    
    # Create a custom query.json in ~/.config/ragit with only a subset of fields
    home_query_json = {
        "max_titles": 16,  # Different from default
        "enable_ii": False  # Different from default
        # Omitting other fields to test partial configuration
    }
    
    # Write the config files
    home_api_path = os.path.join(config_dir, "api.json")
    with open(home_api_path, 'w') as f:
        json.dump(home_api_json, f, indent=2)
    
    home_build_path = os.path.join(config_dir, "build.json")
    with open(home_build_path, 'w') as f:
        json.dump(home_build_json, f, indent=2)
    
    home_query_path = os.path.join(config_dir, "query.json")
    with open(home_query_path, 'w') as f:
        json.dump(home_query_json, f, indent=2)
    
    print("\n--- Testing with ~/.config/ragit/*.json override ---")
    mk_and_cd_tmp_dir()
    
    # Run the ragit init command
    print("Running 'rag init'...")
    cargo_run(["init"])
    
    # Check if the config files exist
    api_json_path = os.path.join(".ragit", "configs", "api.json")
    build_json_path = os.path.join(".ragit", "configs", "build.json")
    query_json_path = os.path.join(".ragit", "configs", "query.json")
    
    assert os.path.exists(api_json_path), f"api.json file does not exist at: {api_json_path}"
    assert os.path.exists(build_json_path), f"build.json file does not exist at: {build_json_path}"
    assert os.path.exists(query_json_path), f"query.json file does not exist at: {query_json_path}"
    
    print(f"Config files exist at: {os.path.join('.ragit', 'configs')}")
    
    # Read the content of the config files
    with open(api_json_path, 'r') as f:
        api_json = json.load(f)
    
    with open(build_json_path, 'r') as f:
        build_json = json.load(f)
    
    with open(query_json_path, 'r') as f:
        query_json = json.load(f)
    
    # Verify that values from ~/.config/ragit/api.json were used
    assert api_json.get('model') == home_api_json['model'], f"Expected model '{home_api_json['model']}', got '{api_json.get('model')}'"
    assert api_json.get('timeout') == home_api_json['timeout'], f"Expected timeout {home_api_json['timeout']}, got {api_json.get('timeout')}"
    assert api_json.get('sleep_between_retries') == home_api_json['sleep_between_retries'], f"Expected sleep_between_retries {home_api_json['sleep_between_retries']}, got {api_json.get('sleep_between_retries')}"
    assert api_json.get('max_retry') == home_api_json['max_retry'], f"Expected max_retry {home_api_json['max_retry']}, got {api_json.get('max_retry')}"
    assert api_json.get('sleep_after_llm_call') == home_api_json['sleep_after_llm_call'], f"Expected sleep_after_llm_call {home_api_json['sleep_after_llm_call']}, got {api_json.get('sleep_after_llm_call')}"
    assert api_json.get('dump_log') == home_api_json['dump_log'], f"Expected dump_log {home_api_json['dump_log']}, got {api_json.get('dump_log')}"
    
    # Verify that specified values from ~/.config/ragit/build.json were used
    assert build_json.get('chunk_size') == home_build_json['chunk_size'], f"Expected chunk_size {home_build_json['chunk_size']}, got {build_json.get('chunk_size')}"
    assert build_json.get('slide_len') == home_build_json['slide_len'], f"Expected slide_len {home_build_json['slide_len']}, got {build_json.get('slide_len')}"
    
    # Verify that default values were used for unspecified fields in build.json
    default_build = {
        "image_size": 2000,
        "min_summary_len": 200,
        "max_summary_len": 1000,
        "strict_file_reader": False,
        "compression_threshold": 2048,
        "compression_level": 3,
    }
    
    assert build_json.get('image_size') == default_build['image_size'], f"Expected default image_size {default_build['image_size']}, got {build_json.get('image_size')}"
    assert build_json.get('min_summary_len') == default_build['min_summary_len'], f"Expected default min_summary_len {default_build['min_summary_len']}, got {build_json.get('min_summary_len')}"
    assert build_json.get('max_summary_len') == default_build['max_summary_len'], f"Expected default max_summary_len {default_build['max_summary_len']}, got {build_json.get('max_summary_len')}"
    assert build_json.get('strict_file_reader') == default_build['strict_file_reader'], f"Expected default strict_file_reader {default_build['strict_file_reader']}, got {build_json.get('strict_file_reader')}"
    assert build_json.get('compression_threshold') == default_build['compression_threshold'], f"Expected default compression_threshold {default_build['compression_threshold']}, got {build_json.get('compression_threshold')}"
    assert build_json.get('compression_level') == default_build['compression_level'], f"Expected default compression_level {default_build['compression_level']}, got {build_json.get('compression_level')}"
    
    # Verify that specified values from ~/.config/ragit/query.json were used
    assert query_json.get('max_titles') == home_query_json['max_titles'], f"Expected max_titles {home_query_json['max_titles']}, got {query_json.get('max_titles')}"
    assert query_json.get('enable_ii') == home_query_json['enable_ii'], f"Expected enable_ii {home_query_json['enable_ii']}, got {query_json.get('enable_ii')}"
    
    # Verify that default values were used for unspecified fields in query.json
    default_query = {
        "max_summaries": 10,
        "max_retrieval": 3,
    }
    
    assert query_json.get('max_summaries') == default_query['max_summaries'], f"Expected default max_summaries {default_query['max_summaries']}, got {query_json.get('max_summaries')}"
    assert query_json.get('max_retrieval') == default_query['max_retrieval'], f"Expected default max_retrieval {default_query['max_retrieval']}, got {query_json.get('max_retrieval')}"
    
    print("SUCCESS: Values from ~/.config/ragit/*.json were correctly used to override defaults!")
    
    # Clean up
    os.remove(home_api_path)
    os.remove(home_build_path)
    os.remove(home_query_path)
    if not os.listdir(config_dir):
        os.rmdir(config_dir)
        parent_dir = os.path.dirname(config_dir)
        if not os.listdir(parent_dir):
            os.rmdir(parent_dir)

if __name__ == "__main__":
    models_init()
    test_home_config_override()
