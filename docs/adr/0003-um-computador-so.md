# Um computador só; a sincronização entre computadores fica engavetada

O app passa a ser usado em um único computador, e a residência tem vários participantes a quem os lançamentos são atribuídos. A sincronização entre computadores dos ADRs 0001 e 0002 foi construída e funcionou em testes, mas na casa a que o app se destina os dois computadores não se alcançavam pela rede local, e a alternativa de manter um servidor na internet é trabalho de operação demais para um app doméstico.

## Consequences

- O app não abre portas nem anuncia nada na rede.
- Um participante pode existir sem senha: ele serve de Responsável por lançamentos, e só entra no app se um Administrador definir uma senha para ele.
- O código de sincronização do `finance-core` e o crate `finance-server` continuam no repositório, com seus testes, mas nenhum deles é usado pelo app. Retomar a ideia é religá-los; quem for mexer neles antes disso deve considerar removê-los.
- As colunas de versão, exclusão marcada e relógio de alterações continuam no banco, sem custo para o uso em um computador.
