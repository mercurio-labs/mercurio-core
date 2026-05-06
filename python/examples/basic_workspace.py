from mercurio import Mercurio


def main() -> None:
    with Mercurio.launch() as backend:
        workspace = backend.open_workspace("../examples")
        result = workspace.compile_project()
        print(result)


if __name__ == "__main__":
    main()
