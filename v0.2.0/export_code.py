import os

# 設定要忽略的資料夾 (垃圾過濾)
IGNORE_DIRS = {
    '.git', '.venv', 'venv', '__pycache__', 'node_modules', 
    'dist', 'build', 'release', '.idea', '.vscode', 
    'coverage', 'htmlcov'
}

# 設定要忽略的檔案
IGNORE_FILES = {
    'package-lock.json', 'yarn.lock', 'pnpm-lock.yaml', 
    'poetry.lock', '.DS_Store', 'export_code.py', 'project_snapshot.txt'
}

# 設定要讀取的副檔名 (只看代碼)
TARGET_EXTENSIONS = {
    '.py', '.js', '.jsx', '.ts', '.tsx', 
    '.css', '.html', '.md', '.json', 
    '.yml', '.yaml', '.sh', '.bat', '.ini', '.txt'
}

OUTPUT_FILE = 'project_snapshot.txt'

def is_text_file(filename):
    return any(filename.endswith(ext) for ext in TARGET_EXTENSIONS)

def main():
    print(f"正在掃描專案並生成 {OUTPUT_FILE} ...")
    
    with open(OUTPUT_FILE, 'w', encoding='utf-8') as outfile:
        # 寫入檔頭
        outfile.write(f"# GALROON PROJECT SNAPSHOT\n")
        outfile.write(f"# 此檔案包含專案所有核心代碼\n")
        outfile.write("="*50 + "\n\n")

        for root, dirs, files in os.walk('.'):
            # 過濾忽略的目錄
            dirs[:] = [d for d in dirs if d not in IGNORE_DIRS]
            
            for file in files:
                if file in IGNORE_FILES:
                    continue
                
                if not is_text_file(file):
                    continue

                file_path = os.path.join(root, file)
                
                # 為了顯示漂亮，把路徑分隔符統一
                display_path = file_path.replace('\\', '/')
                
                try:
                    with open(file_path, 'r', encoding='utf-8') as infile:
                        content = infile.read()
                        
                        # 寫入檔案分隔標記
                        outfile.write(f"\n{'='*50}\n")
                        outfile.write(f"FILE: {display_path}\n")
                        outfile.write(f"{'='*50}\n")
                        outfile.write(content + "\n")
                        
                        print(f"已加入: {display_path}")
                except Exception as e:
                    print(f"跳過 (讀取錯誤): {display_path} - {e}")

    print(f"\n完成！請將 {OUTPUT_FILE} 上傳給 AI。")

if __name__ == '__main__':
    main()